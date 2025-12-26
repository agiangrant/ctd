// Desktop entry point for macOS, Linux, and Windows
//
// Build:
//   go build -o myapp ./examples/unified
//
// The same app/ package is used for mobile builds via gomobile.
package main

import (
	"log"
	"runtime"

	"github.com/agiangrant/ctd/examples/unified/app"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	application := app.New()
	if err := application.Run(); err != nil {
		log.Fatal(err)
	}
}
