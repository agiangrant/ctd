//go:build !js

package ffi

import (
	"encoding/binary"
	"math"
	"sync"
)

// TransportMode specifies how Go communicates with the Rust engine.
type TransportMode int

const (
	// TransportSharedMemory uses binary protocol with dual buffers for maximum performance.
	// This is the default for production use.
	TransportSharedMemory TransportMode = iota

	// TransportFFI uses direct CGO calls with JSON encoding for easier debugging.
	// Use this during development when you need to trace individual calls.
	TransportFFI
)

// Command types for the binary protocol.
// These must match the Rust side exactly.
// Using u16 with 256-spacing between groups to allow room for growth.
type CommandType uint16

const (
	// Text measurement commands (0x0000 - 0x00FF)
	CmdMeasureText         CommandType = 0x0000
	CmdMeasureTextBatch    CommandType = 0x0001
	CmdMeasureTextToCursor CommandType = 0x0002
	CmdMeasureTextWithFont CommandType = 0x0003

	// Image commands (0x0100 - 0x01FF)
	CmdLoadImage      CommandType = 0x0100
	CmdLoadImageFile  CommandType = 0x0101
	CmdUnloadImage    CommandType = 0x0102
	CmdGetTextureSize CommandType = 0x0103

	// Render commands (0x0200 - 0x02FF)
	CmdRenderFrame CommandType = 0x0200

	// System queries (0x0300 - 0x03FF)
	CmdGetScaleFactor CommandType = 0x0300
	CmdGetDarkMode    CommandType = 0x0301

	// Audio playback commands (0x0400 - 0x04FF)
	CmdAudioCreate     CommandType = 0x0400
	CmdAudioDestroy    CommandType = 0x0401
	CmdAudioLoadURL    CommandType = 0x0402
	CmdAudioLoadFile   CommandType = 0x0403
	CmdAudioPlay       CommandType = 0x0404
	CmdAudioPause      CommandType = 0x0405
	CmdAudioStop       CommandType = 0x0406
	CmdAudioSeek       CommandType = 0x0407
	CmdAudioSetVolume  CommandType = 0x0408
	CmdAudioSetLooping CommandType = 0x0409
	CmdAudioGetState   CommandType = 0x040A
	CmdAudioGetTime    CommandType = 0x040B
	CmdAudioGetInfo    CommandType = 0x040C
	CmdAudioGetVolume  CommandType = 0x040D
	CmdAudioIsLooping  CommandType = 0x040E
	CmdAudioUpdate     CommandType = 0x040F

	// Audio input commands (0x0500 - 0x05FF)
	CmdAudioInputCreate            CommandType = 0x0500
	CmdAudioInputDestroy           CommandType = 0x0501
	CmdAudioInputRequestPermission CommandType = 0x0502
	CmdAudioInputHasPermission     CommandType = 0x0503
	CmdAudioInputListDevices       CommandType = 0x0504
	CmdAudioInputOpen              CommandType = 0x0505
	CmdAudioInputStart             CommandType = 0x0506
	CmdAudioInputStop              CommandType = 0x0507
	CmdAudioInputClose             CommandType = 0x0508
	CmdAudioInputGetLevel          CommandType = 0x0509
	CmdAudioInputGetState          CommandType = 0x050A

	// Video playback commands (0x0600 - 0x06FF)
	CmdVideoCreate       CommandType = 0x0600
	CmdVideoDestroy      CommandType = 0x0601
	CmdVideoLoadURL      CommandType = 0x0602
	CmdVideoLoadFile     CommandType = 0x0603
	CmdVideoInitStream   CommandType = 0x0604
	CmdVideoPushFrame    CommandType = 0x0605
	CmdVideoPlay         CommandType = 0x0606
	CmdVideoPause        CommandType = 0x0607
	CmdVideoSeek         CommandType = 0x0608
	CmdVideoSetLooping   CommandType = 0x0609
	CmdVideoSetMuted     CommandType = 0x060A
	CmdVideoSetVolume    CommandType = 0x060B
	CmdVideoGetState     CommandType = 0x060C
	CmdVideoGetTime      CommandType = 0x060D
	CmdVideoGetInfo      CommandType = 0x060E
	CmdVideoUpdate       CommandType = 0x060F
	CmdVideoGetTextureID CommandType = 0x0610

	// Video input commands (0x0700 - 0x07FF)
	CmdVideoInputCreate            CommandType = 0x0700
	CmdVideoInputDestroy           CommandType = 0x0701
	CmdVideoInputRequestPermission CommandType = 0x0702
	CmdVideoInputHasPermission     CommandType = 0x0703
	CmdVideoInputListDevices       CommandType = 0x0704
	CmdVideoInputOpen              CommandType = 0x0705
	CmdVideoInputStart             CommandType = 0x0706
	CmdVideoInputStop              CommandType = 0x0707
	CmdVideoInputClose             CommandType = 0x0708
	CmdVideoInputGetState          CommandType = 0x0709
	CmdVideoInputGetDimensions     CommandType = 0x070A
	CmdVideoInputGetFrameTexture   CommandType = 0x070B

	// Clipboard commands (0x0800 - 0x08FF)
	CmdClipboardGet CommandType = 0x0800
	CmdClipboardSet CommandType = 0x0801

	// App lifecycle (0xFF00 - 0xFFFF)
	CmdRequestRedraw CommandType = 0xFF00
	CmdRequestExit   CommandType = 0xFF01
)

// Response types for the binary protocol.
type ResponseType uint8

const (
	RespSuccess      ResponseType = 0
	RespError        ResponseType = 1
	RespFloat32      ResponseType = 2
	RespInt32        ResponseType = 3
	RespUint32       ResponseType = 4
	RespUint64       ResponseType = 5
	RespString       ResponseType = 6
	RespBytes        ResponseType = 7
	RespBool         ResponseType = 8
	RespFloat32Array ResponseType = 9
	RespUint32Pair   ResponseType = 10 // For texture size, dimensions
	RespUint32Triple ResponseType = 11 // For audio info (duration, sample_rate, channels)
	RespVideoInfo    ResponseType = 12 // For video info (width, height, duration)
)

// Transport is the interface for Go-Rust communication.
// Both FFI and SharedMemory modes implement this interface.
type Transport interface {
	// Execute sends a command and receives a response.
	// In FFI mode, this makes a direct CGO call.
	// In SharedMemory mode, this writes to the request buffer and reads from response buffer.
	Execute(cmd CommandType, payload []byte) (ResponseType, []byte, error)

	// ExecuteBatch sends multiple commands and receives multiple responses.
	// In FFI mode, this calls Execute in a loop.
	// In SharedMemory mode, this batches all commands into a single FFI call.
	ExecuteBatch(cmds []CommandType, payloads [][]byte) ([]ResponseType, [][]byte, error)

	// Flush ensures all pending commands are processed (for SharedMemory mode).
	// No-op for FFI mode.
	Flush() error

	// Mode returns the transport mode.
	Mode() TransportMode

	// Close releases any resources held by the transport.
	Close() error
}

// activeTransport is the global transport instance.
var (
	activeTransport Transport
	transportMu     sync.RWMutex
	transportMode   TransportMode = TransportSharedMemory // default
)

// SetTransportMode sets the transport mode. Must be called before InitTransport.
func SetTransportMode(mode TransportMode) {
	transportMu.Lock()
	defer transportMu.Unlock()
	transportMode = mode
}

// GetTransportMode returns the current transport mode.
func GetTransportMode() TransportMode {
	transportMu.RLock()
	defer transportMu.RUnlock()
	return transportMode
}

// InitTransport initializes the transport based on the configured mode.
// This is called automatically when the app starts.
func InitTransport() error {
	transportMu.Lock()
	defer transportMu.Unlock()

	if activeTransport != nil {
		return nil // Already initialized
	}

	switch transportMode {
	case TransportFFI:
		activeTransport = newFFITransport()
	case TransportSharedMemory:
		activeTransport = newSharedMemoryTransport()
	}

	return nil
}

// GetTransport returns the active transport, initializing if needed.
func GetTransport() Transport {
	transportMu.RLock()
	if activeTransport != nil {
		transportMu.RUnlock()
		return activeTransport
	}
	transportMu.RUnlock()

	// Need to initialize
	InitTransport()

	transportMu.RLock()
	defer transportMu.RUnlock()
	return activeTransport
}

// CloseTransport closes the active transport.
func CloseTransport() error {
	transportMu.Lock()
	defer transportMu.Unlock()

	if activeTransport != nil {
		err := activeTransport.Close()
		activeTransport = nil
		return err
	}
	return nil
}

// Binary encoding helpers

// PutUint32 writes a uint32 to the buffer in little-endian.
func PutUint32(buf []byte, v uint32) {
	binary.LittleEndian.PutUint32(buf, v)
}

// GetUint32 reads a uint32 from the buffer in little-endian.
func GetUint32(buf []byte) uint32 {
	return binary.LittleEndian.Uint32(buf)
}

// PutFloat32 writes a float32 to the buffer.
func PutFloat32(buf []byte, v float32) {
	binary.LittleEndian.PutUint32(buf, uint32FromFloat32(v))
}

// GetFloat32 reads a float32 from the buffer.
func GetFloat32(buf []byte) float32 {
	return float32FromUint32(binary.LittleEndian.Uint32(buf))
}

// PutString writes a length-prefixed string to the buffer.
// Returns the number of bytes written.
func PutString(buf []byte, s string) int {
	binary.LittleEndian.PutUint32(buf, uint32(len(s)))
	copy(buf[4:], s)
	return 4 + len(s)
}

// GetString reads a length-prefixed string from the buffer.
// Returns the string and number of bytes consumed.
func GetString(buf []byte) (string, int) {
	length := binary.LittleEndian.Uint32(buf)
	return string(buf[4 : 4+length]), 4 + int(length)
}

// uint32FromFloat32 converts float32 to uint32 bits.
func uint32FromFloat32(f float32) uint32 {
	return math.Float32bits(f)
}

// float32FromUint32 converts uint32 bits to float32.
func float32FromUint32(u uint32) float32 {
	return math.Float32frombits(u)
}
