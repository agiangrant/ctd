// Comprehensive Advanced Layout Example
// Tests: flex-basis, align-self, order, fractional widths/heights, percentage sizing
// No explicit sizing - let the layout engine do its job
package main

import (
	"log"
	"runtime"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)
	tree := loop.Tree()

	root := buildUI()
	tree.SetRoot(root)

	loop.OnResize(func(width, height float32) {
		root.SetSize(width, height)
	})

	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Advanced Layout Demo"
	appConfig.Width = 1000
	appConfig.Height = 800

	log.Println("Starting Advanced Layout Demo...")
	log.Println("  Testing: flex-basis, align-self, order, fractional widths")
	log.Println("Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI() *retained.Widget {
	return retained.VStack("bg-gray-900 w-full h-full p-4 gap-4 overflow-y-auto",
		// Header
		retained.Text("Advanced Layout Features Demo", "text-white text-2xl"),
		retained.Text("Resize window below 768px to see responsive layout (md:flex-col)", "text-gray-400 text-sm"),

		// Sticky test at the top for visibility
		retained.Text("Sticky Test (scroll inside the box below):", "text-gray-400 text-sm"),
		retained.VStack("bg-gray-700 rounded h-24 w-full overflow-y-auto",
			retained.Text("STICKY HEADER", "text-white text-sm bg-yellow-600 sticky top-0 px-2 py-1"),
			retained.Text("Item 1 - scroll down to test sticky", "text-gray-300 text-xs px-2"),
			retained.Text("Item 2", "text-gray-300 text-xs px-2"),
			retained.Text("Item 3", "text-gray-300 text-xs px-2"),
			retained.Text("Item 4", "text-gray-300 text-xs px-2"),
			retained.Text("Item 5", "text-gray-300 text-xs px-2"),
			retained.Text("Item 6", "text-gray-300 text-xs px-2"),
			retained.Text("Item 7", "text-gray-300 text-xs px-2"),
			retained.Text("Item 8", "text-gray-300 text-xs px-2"),
		),

		// Main content - responsive two columns
		// flex-row on small screens, flex-col on md+ (768px+)
		// Use Flex for responsive direction based on breakpoints
		retained.Flex("flex flex-col md:flex-row md:w-full flex-1 gap-4",
			// Left column (or top section on md+)
			retained.VStack("md:w-1/2 w-full gap-4",
				buildFractionalWidthSection(),
				buildFlexWrapSection(),
				buildAlignSelfSection(),
				buildPositionSection(),
			),
			// Right column (or bottom section on md+)
			retained.VStack("md:w-1/2 w-full gap-4",
				buildFlexBasisSection(),
				buildOrderSection(),
				buildTextAlignSection(),
			),
		),
	)
}

// buildFractionalWidthSection demonstrates w-1/2, w-1/3, w-2/3, etc.
func buildFractionalWidthSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		retained.Text("Fractional Widths", "text-yellow-400 text-lg"),
		retained.Text("w-1/2, w-1/3, w-2/3, w-1/4, w-3/4", "text-gray-400 text-xs"),

		// 1/2 + 1/2
		retained.Text("w-1/2 + w-1/2", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			retained.Container("bg-blue-500 rounded w-1/2 h-8"),
			retained.Container("bg-blue-400 rounded w-1/2 h-8"),
		),

		// 1/3 + 1/3 + 1/3
		retained.Text("w-1/3 + w-1/3 + w-1/3", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			retained.Container("bg-green-500 rounded w-1/3 h-8"),
			retained.Container("bg-green-400 rounded w-1/3 h-8"),
			retained.Container("bg-green-300 rounded w-1/3 h-8"),
		),

		// 1/4 + 3/4
		retained.Text("w-1/4 + w-3/4 (sidebar + content)", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			retained.Container("bg-purple-500 rounded w-1/4 h-8"),
			retained.Container("bg-purple-300 rounded w-3/4 h-8"),
		),
	)
}

// buildFlexWrapSection demonstrates flex-wrap classes
func buildFlexWrapSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		retained.Text("Flex Wrap", "text-yellow-400 text-lg"),
		retained.Text("flex-wrap, flex-nowrap, flex-wrap-reverse", "text-gray-400 text-xs"),

		// flex-wrap: items wrap to new lines when they exceed container width
		retained.Text("flex-wrap (items wrap to new lines)", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-wrap",
			retained.Container("bg-blue-500 rounded w-20 h-8"),
			retained.Container("bg-blue-400 rounded w-20 h-8"),
			retained.Container("bg-blue-300 rounded w-20 h-8"),
			retained.Container("bg-blue-500 rounded w-20 h-8"),
			retained.Container("bg-blue-400 rounded w-20 h-8"),
			retained.Container("bg-blue-300 rounded w-20 h-8"),
			retained.Container("bg-blue-500 rounded w-20 h-8"),
			retained.Container("bg-blue-400 rounded w-20 h-8"),
		),

		// flex-nowrap: items stay on one line (default, may overflow)
		retained.Text("flex-nowrap (single line, may overflow)", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-nowrap overflow-x-auto",
			retained.Container("bg-green-500 rounded w-20 h-8"),
			retained.Container("bg-green-400 rounded w-20 h-8"),
			retained.Container("bg-green-300 rounded w-20 h-8"),
			retained.Container("bg-green-500 rounded w-20 h-8"),
			retained.Container("bg-green-400 rounded w-20 h-8"),
			retained.Container("bg-green-300 rounded w-20 h-8"),
			retained.Container("bg-green-500 rounded w-20 h-8"),
			retained.Container("bg-green-400 rounded w-20 h-8"),
		),

		// flex-wrap-reverse: items wrap in reverse order
		retained.Text("flex-wrap-reverse (wrap, lines reversed)", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-wrap-reverse",
			retained.Container("bg-purple-500 rounded w-20 h-8"),
			retained.Container("bg-purple-400 rounded w-20 h-8"),
			retained.Container("bg-purple-300 rounded w-20 h-8"),
			retained.Container("bg-purple-500 rounded w-20 h-8"),
			retained.Container("bg-purple-400 rounded w-20 h-8"),
			retained.Container("bg-purple-300 rounded w-20 h-8"),
			retained.Container("bg-purple-500 rounded w-20 h-8"),
			retained.Container("bg-purple-400 rounded w-20 h-8"),
		),

		// flex-wrap with varying heights
		retained.Text("flex-wrap with varying heights", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-wrap items-start",
			retained.Container("bg-orange-500 rounded w-16 h-8"),
			retained.Container("bg-orange-400 rounded w-16 h-12"),
			retained.Container("bg-orange-300 rounded w-16 h-6"),
			retained.Container("bg-orange-500 rounded w-16 h-10"),
			retained.Container("bg-orange-400 rounded w-16 h-8"),
			retained.Container("bg-orange-300 rounded w-16 h-14"),
			retained.Container("bg-orange-500 rounded w-16 h-8"),
			retained.Container("bg-orange-400 rounded w-16 h-10"),
		),
	)
}

// buildFlexBasisSection demonstrates flex-basis classes
func buildFlexBasisSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		retained.Text("Flex Basis", "text-yellow-400 text-lg"),
		retained.Text("basis-0, basis-auto, basis-1/2, basis-full", "text-gray-400 text-xs"),

		// basis-0 with flex-grow: items share space equally regardless of content
		retained.Text("basis-0 flex-grow (equal distribution)", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			retained.VStack("bg-red-500 rounded p-2 basis-0 flex-grow",
				retained.Text("Short", "text-white text-xs"),
			),
			retained.VStack("bg-red-400 rounded p-2 basis-0 flex-grow",
				retained.Text("Medium text", "text-white text-xs"),
			),
			retained.VStack("bg-red-300 rounded p-2 basis-0 flex-grow",
				retained.Text("Longer content", "text-white text-xs"),
			),
		),

		// basis-1/2: item starts at 50% width
		retained.Text("basis-1/2 + flex-grow items", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			retained.Container("bg-yellow-500 rounded basis-1/2 h-8"),
			retained.Container("bg-yellow-400 rounded flex-grow h-8"),
			retained.Container("bg-yellow-300 rounded flex-grow h-8"),
		),

		// basis-1/4: each item takes 1/4 width
		retained.Text("basis-1/4 items", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			retained.Container("bg-cyan-500 rounded basis-1/4 h-8"),
			retained.Container("bg-cyan-400 rounded basis-1/4 h-8"),
			retained.Container("bg-cyan-300 rounded basis-1/4 h-8"),
			retained.Container("bg-cyan-200 rounded basis-1/4 h-8"),
		),
	)
}

// buildAlignSelfSection demonstrates align-self override
func buildAlignSelfSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		retained.Text("Align Self", "text-yellow-400 text-lg"),
		retained.Text("Parent: items-center | Children override", "text-gray-400 text-xs"),

		retained.HStack("bg-gray-700 rounded gap-2 p-2 items-center h-24",
			retained.VStack("bg-pink-500 rounded p-2 self-start",
				retained.Text("self-start", "text-white text-xs"),
			),
			retained.VStack("bg-pink-400 rounded p-2 self-center",
				retained.Text("self-center", "text-white text-xs"),
			),
			retained.VStack("bg-pink-300 rounded p-2 self-end",
				retained.Text("self-end", "text-white text-xs"),
			),
			retained.VStack("bg-pink-200 rounded p-2 self-stretch",
				retained.Text("self-stretch", "text-gray-800 text-xs"),
			),
			retained.VStack("bg-pink-100 rounded p-2",
				retained.Text("(default)", "text-gray-800 text-xs"),
			),
		),
	)
}

// buildPositionSection demonstrates CSS positioning (relative, absolute, sticky)
func buildPositionSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-2",
		retained.Text("Positioning", "text-yellow-400 text-lg"),

		// Relative positioning example
		retained.Text("relative + top-2 left-4", "text-gray-400 text-xs"),
		retained.HStack("bg-gray-700 rounded gap-2 p-2 h-12",
			retained.Container("bg-blue-500 rounded w-10 h-6"),
			retained.Container("bg-blue-400 rounded w-10 h-6 relative top-2 left-4"),
			retained.Container("bg-blue-500 rounded w-10 h-6"),
		),

		// Absolute positioning example
		retained.Text("absolute children in relative parent", "text-gray-400 text-xs"),
		retained.ZStack("bg-gray-700 rounded p-2 h-16 w-full relative",
			retained.Text("Content", "text-white text-xs"),
			retained.VStack("bg-red-500 rounded px-1 absolute top-1 right-1",
				retained.Text("TR", "text-white text-xs"),
			),
			retained.VStack("bg-green-500 rounded px-1 absolute bottom-1 left-1",
				retained.Text("BL", "text-white text-xs"),
			),
		),

		// Absolute inset (all edges)
		retained.Text("absolute inset (all edges set)", "text-gray-400 text-xs"),
		retained.ZStack("bg-gray-700 rounded h-12 w-full relative",
			retained.Container("bg-purple-500 rounded absolute top-2 right-2 bottom-2 left-2"),
		),

		// Nested containing blocks
		retained.Text("nested: absolute finds outer relative", "text-gray-400 text-xs"),
		retained.ZStack("bg-gray-700 rounded p-2 h-20 w-full relative",
			retained.Text("Outer", "text-white text-xs"),
			retained.VStack("bg-gray-600 rounded p-2 w-3/4 h-10",
				retained.Text("Inner (static)", "text-gray-300 text-xs"),
				retained.VStack("bg-cyan-500 rounded px-1 absolute bottom-1 right-1",
					retained.Text("In outer!", "text-white text-xs"),
				),
			),
		),

		// Sticky positioning
		retained.Text("sticky header (scroll to test)", "text-gray-400 text-xs"),
		retained.VStack("bg-gray-700 rounded h-16 w-full overflow-y-auto",
			retained.Text("Sticky", "text-white text-xs bg-yellow-600 sticky top-0 px-2"),
			retained.Text("Item 1", "text-gray-300 text-xs px-2"),
			retained.Text("Item 2", "text-gray-300 text-xs px-2"),
			retained.Text("Item 3", "text-gray-300 text-xs px-2"),
			retained.Text("Item 4", "text-gray-300 text-xs px-2"),
			retained.Text("Item 5", "text-gray-300 text-xs px-2"),
		),
	)
}

// buildTextAlignSection demonstrates text alignment classes
func buildTextAlignSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		retained.Text("Text Alignment", "text-yellow-400 text-lg"),
		retained.Text("text-left, text-center, text-right, text-justify", "text-gray-400 text-xs"),

		// Basic alignments
		retained.VStack("bg-gray-700 rounded p-2 gap-2 w-full",
			retained.Text("Left aligned (default)", "text-white text-sm text-left w-full"),
			retained.Text("Center aligned text", "text-white text-sm text-center w-full"),
			retained.Text("Right aligned text", "text-white text-sm text-right w-full"),
		),

		// Responsive alignment
		retained.Text("Responsive: text-left md:text-center lg:text-right", "text-gray-400 text-xs"),
		retained.VStack("bg-gray-700 rounded p-2 w-full",
			retained.Text("Resize window to see alignment change", "text-cyan-300 text-sm text-left md:text-center lg:text-right w-full"),
		),

		// start/end aliases
		retained.Text("text-start / text-end (LTR aliases)", "text-gray-400 text-xs"),
		retained.VStack("bg-gray-700 rounded p-2 gap-2 w-full",
			retained.Text("text-start (same as left)", "text-white text-sm text-start w-full"),
			retained.Text("text-end (same as right)", "text-white text-sm text-end w-full"),
		),

		// Justified text
		retained.Text("text-justify (spreads words to fill width)", "text-gray-400 text-xs"),
		retained.VStack("bg-gray-700 rounded p-2 w-full",
			retained.Text("This is justified text that spans multiple lines. The words are spread out evenly so that each line fills the full width of the container, creating clean edges on both sides.", "text-white text-sm text-justify w-full"),
		),
	)
}

// buildOrderSection demonstrates order classes for visual reordering
func buildOrderSection() *retained.Widget {
	return retained.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		retained.Text("Order (visual reordering)", "text-yellow-400 text-lg"),
		retained.Text("DOM order: A, B, C, D, E", "text-gray-400 text-xs"),
		retained.Text("Visual: C(first), A(1), B(2), E(3), D(last)", "text-gray-400 text-xs"),

		retained.HStack("bg-gray-700 rounded gap-2 p-2",
			// A: order-1
			retained.VStack("bg-orange-500 rounded p-2 order-1",
				retained.Text("A", "text-white text-lg"),
				retained.Text("order-1", "text-white text-xs"),
			),
			// B: order-2
			retained.VStack("bg-orange-400 rounded p-2 order-2",
				retained.Text("B", "text-white text-lg"),
				retained.Text("order-2", "text-white text-xs"),
			),
			// C: order-first
			retained.VStack("bg-orange-600 rounded p-2 order-first",
				retained.Text("C", "text-white text-lg"),
				retained.Text("order-first", "text-white text-xs"),
			),
			// D: order-last
			retained.VStack("bg-orange-300 rounded p-2 order-last",
				retained.Text("D", "text-gray-800 text-lg"),
				retained.Text("order-last", "text-gray-800 text-xs"),
			),
			// E: order-3
			retained.VStack("bg-orange-200 rounded p-2 order-3",
				retained.Text("E", "text-gray-800 text-lg"),
				retained.Text("order-3", "text-gray-800 text-xs"),
			),
		),
	)
}
