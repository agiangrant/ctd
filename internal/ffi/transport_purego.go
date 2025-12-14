package ffi

import (
	"encoding/binary"
	"errors"
	"sync"
	"unsafe"
)

// purego implementations of transports when CGO is disabled.
// Uses purego FFI calls instead of CGO.

// Initial buffer sizes - will grow as needed
const (
	puregoInitialRequestBufferSize  = 64 * 1024        // 64KB
	puregoInitialResponseBufferSize = 64 * 1024        // 64KB
	puregoMaxBufferSize             = 64 * 1024 * 1024 // 64MB max
)

type ffiTransport struct{}

func newFFITransport() *ffiTransport {
	return &ffiTransport{}
}

func (t *ffiTransport) Mode() TransportMode {
	return TransportFFI
}

func (t *ffiTransport) Close() error {
	return nil
}

func (t *ffiTransport) Flush() error {
	return nil
}

func (t *ffiTransport) Execute(cmd CommandType, payload []byte) (ResponseType, []byte, error) {
	// In purego mode, FFI transport delegates to shared memory transport
	// since we have the batch execution function available
	respTypes, respPayloads, err := t.ExecuteBatch([]CommandType{cmd}, [][]byte{payload})
	if err != nil {
		return RespError, nil, err
	}
	return respTypes[0], respPayloads[0], nil
}

func (t *ffiTransport) ExecuteBatch(cmds []CommandType, payloads [][]byte) ([]ResponseType, [][]byte, error) {
	// Delegate to shared memory transport implementation
	shm := newSharedMemoryTransport()
	return shm.ExecuteBatch(cmds, payloads)
}

// sharedMemoryTransport implements Transport using dual buffers and purego FFI.
type sharedMemoryTransport struct {
	mu sync.Mutex

	// Request buffer - Go writes commands here
	requestBuf []byte
	requestLen int

	// Response buffer - Rust writes results here
	responseBuf []byte
	responseLen int
}

func newSharedMemoryTransport() *sharedMemoryTransport {
	return &sharedMemoryTransport{
		requestBuf:  make([]byte, puregoInitialRequestBufferSize),
		responseBuf: make([]byte, puregoInitialResponseBufferSize),
	}
}

func (t *sharedMemoryTransport) Mode() TransportMode {
	return TransportSharedMemory
}

func (t *sharedMemoryTransport) Close() error {
	t.mu.Lock()
	defer t.mu.Unlock()
	t.requestBuf = nil
	t.responseBuf = nil
	return nil
}

func (t *sharedMemoryTransport) Flush() error {
	return nil
}

func (t *sharedMemoryTransport) Execute(cmd CommandType, payload []byte) (ResponseType, []byte, error) {
	respTypes, respPayloads, err := t.ExecuteBatch([]CommandType{cmd}, [][]byte{payload})
	if err != nil {
		return RespError, nil, err
	}
	return respTypes[0], respPayloads[0], nil
}

func (t *sharedMemoryTransport) ExecuteBatch(cmds []CommandType, payloads [][]byte) ([]ResponseType, [][]byte, error) {
	t.mu.Lock()
	defer t.mu.Unlock()

	if fnExecuteBatch == nil {
		return nil, nil, errors.New("batch execution not available - library not initialized")
	}

	if len(cmds) == 0 {
		return nil, nil, nil
	}

	// Calculate required request buffer size
	// Format: count(4) + [cmd(2) + payloadLen(4) + payload]...
	reqSize := 4 // count
	for i := range cmds {
		reqSize += 2 + 4 + len(payloads[i]) // cmd(2) + len(4) + payload
	}

	// Grow request buffer if needed
	if reqSize > len(t.requestBuf) {
		newSize := len(t.requestBuf) * 2
		for newSize < reqSize {
			newSize *= 2
		}
		if newSize > puregoMaxBufferSize {
			return nil, nil, errors.New("request buffer would exceed max size")
		}
		t.requestBuf = make([]byte, newSize)
	}

	// Build request: count(4) + [cmd(2) + payloadLen(4) + payload]...
	binary.LittleEndian.PutUint32(t.requestBuf[0:4], uint32(len(cmds)))
	offset := 4

	for i, cmd := range cmds {
		// Command type (2 bytes)
		binary.LittleEndian.PutUint16(t.requestBuf[offset:offset+2], uint16(cmd))
		offset += 2

		// Payload length (4 bytes)
		binary.LittleEndian.PutUint32(t.requestBuf[offset:offset+4], uint32(len(payloads[i])))
		offset += 4

		// Payload
		copy(t.requestBuf[offset:], payloads[i])
		offset += len(payloads[i])
	}

	t.requestLen = offset

	// Call Rust via purego FFI
	var responseLen uintptr
	result := fnExecuteBatch(
		uintptr(unsafe.Pointer(&t.requestBuf[0])),
		uintptr(t.requestLen),
		uintptr(unsafe.Pointer(&t.responseBuf[0])),
		uintptr(len(t.responseBuf)),
		uintptr(unsafe.Pointer(&responseLen)),
	)

	if result < 0 {
		if result == -2 {
			// Response buffer too small, grow and retry
			newSize := len(t.responseBuf) * 2
			if newSize > puregoMaxBufferSize {
				return nil, nil, errors.New("response buffer would exceed max size")
			}
			t.responseBuf = make([]byte, newSize)
			// Retry
			result = fnExecuteBatch(
				uintptr(unsafe.Pointer(&t.requestBuf[0])),
				uintptr(t.requestLen),
				uintptr(unsafe.Pointer(&t.responseBuf[0])),
				uintptr(len(t.responseBuf)),
				uintptr(unsafe.Pointer(&responseLen)),
			)
			if result < 0 {
				return nil, nil, errors.New("batch execution failed after retry")
			}
		} else {
			return nil, nil, errors.New("batch execution failed")
		}
	}

	t.responseLen = int(responseLen)

	// Parse responses: count(4) + [type(1) + payloadLen(4) + payload]...
	if t.responseLen < 4 {
		return nil, nil, errors.New("response too short")
	}

	respCount := int(binary.LittleEndian.Uint32(t.responseBuf[0:4]))
	respOffset := 4

	respTypes := make([]ResponseType, respCount)
	respPayloads := make([][]byte, respCount)

	for i := 0; i < respCount; i++ {
		if respOffset+5 > t.responseLen {
			return nil, nil, errors.New("response truncated")
		}

		// Response type (1 byte)
		respTypes[i] = ResponseType(t.responseBuf[respOffset])
		respOffset++

		// Payload length (4 bytes)
		payloadLen := int(binary.LittleEndian.Uint32(t.responseBuf[respOffset : respOffset+4]))
		respOffset += 4

		if respOffset+payloadLen > t.responseLen {
			return nil, nil, errors.New("response payload truncated")
		}

		// Copy payload (don't reference buffer directly as it may be reused)
		respPayloads[i] = make([]byte, payloadLen)
		copy(respPayloads[i], t.responseBuf[respOffset:respOffset+payloadLen])
		respOffset += payloadLen
	}

	return respTypes, respPayloads, nil
}
