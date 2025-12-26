// Comprehensive Advanced Layout Example
// Tests: flex-basis, align-self, order, fractional widths/heights, percentage sizing
// No explicit sizing - let the layout engine do its job
package main

import (
	"log"
	"runtime"

	"github.com/agiangrant/ctd/internal/ffi"
	"github.com/agiangrant/ctd"
)

func init() {
	runtime.LockOSThread()
}

func main() {
	config := ctd.DefaultLoopConfig()
	loop := ctd.NewLoop(config)
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

func buildUI() *ctd.Widget {
	return ctd.VStack("bg-gray-900 w-full h-full p-4 gap-4 overflow-y-auto",
		// Header
		ctd.Text("Advanced Layout Features Demo", "text-white text-2xl"),
		ctd.Text("Resize window below 768px to see responsive layout (md:flex-col)", "text-gray-400 text-sm"),

		// Sticky test at the top for visibility
		ctd.Text("Sticky Test (scroll inside the box below):", "text-gray-400 text-sm"),
		ctd.VStack("bg-gray-700 rounded h-24 w-full overflow-y-auto",
			ctd.Text("STICKY HEADER", "text-white text-sm bg-yellow-600 sticky top-0 px-2 py-1"),
			ctd.Text("Item 1 - scroll down to test sticky", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 2", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 3", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 4", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 5", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 6", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 7", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 8", "text-gray-300 text-xs px-2"),
		),

		// Main content - responsive two columns
		// flex-row on small screens, flex-col on md+ (768px+)
		// Use Flex for responsive direction based on breakpoints
		ctd.Flex("flex flex-col md:flex-row md:w-full flex-1 gap-4",
			// Left column (or top section on md+)
			ctd.VStack("md:w-1/2 w-full gap-4",
				buildFractionalWidthSection(),
				buildFlexWrapSection(),
				buildAlignSelfSection(),
				buildPositionSection(),
			),
			// Right column (or bottom section on md+)
			ctd.VStack("md:w-1/2 w-full gap-4",
				buildFlexBasisSection(),
				buildOrderSection(),
				buildTextAlignSection(),
			),
		),
	)
}

// buildFractionalWidthSection demonstrates w-1/2, w-1/3, w-2/3, etc.
func buildFractionalWidthSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text("Fractional Widths", "text-yellow-400 text-lg"),
		ctd.Text("w-1/2, w-1/3, w-2/3, w-1/4, w-3/4", "text-gray-400 text-xs"),

		// 1/2 + 1/2
		ctd.Text("w-1/2 + w-1/2", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			ctd.Container("bg-blue-500 rounded w-1/2 h-8"),
			ctd.Container("bg-blue-400 rounded w-1/2 h-8"),
		),

		// 1/3 + 1/3 + 1/3
		ctd.Text("w-1/3 + w-1/3 + w-1/3", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			ctd.Container("bg-green-500 rounded w-1/3 h-8"),
			ctd.Container("bg-green-400 rounded w-1/3 h-8"),
			ctd.Container("bg-green-300 rounded w-1/3 h-8"),
		),

		// 1/4 + 3/4
		ctd.Text("w-1/4 + w-3/4 (sidebar + content)", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			ctd.Container("bg-purple-500 rounded w-1/4 h-8"),
			ctd.Container("bg-purple-300 rounded w-3/4 h-8"),
		),
	)
}

// buildFlexWrapSection demonstrates flex-wrap classes
func buildFlexWrapSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text("Flex Wrap", "text-yellow-400 text-lg"),
		ctd.Text("flex-wrap, flex-nowrap, flex-wrap-reverse", "text-gray-400 text-xs"),

		// flex-wrap: items wrap to new lines when they exceed container width
		ctd.Text("flex-wrap (items wrap to new lines)", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-wrap",
			ctd.Container("bg-blue-500 rounded w-20 h-8"),
			ctd.Container("bg-blue-400 rounded w-20 h-8"),
			ctd.Container("bg-blue-300 rounded w-20 h-8"),
			ctd.Container("bg-blue-500 rounded w-20 h-8"),
			ctd.Container("bg-blue-400 rounded w-20 h-8"),
			ctd.Container("bg-blue-300 rounded w-20 h-8"),
			ctd.Container("bg-blue-500 rounded w-20 h-8"),
			ctd.Container("bg-blue-400 rounded w-20 h-8"),
		),

		// flex-nowrap: items stay on one line (default, may overflow)
		ctd.Text("flex-nowrap (single line, may overflow)", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-nowrap overflow-x-auto",
			ctd.Container("bg-green-500 rounded w-20 h-8"),
			ctd.Container("bg-green-400 rounded w-20 h-8"),
			ctd.Container("bg-green-300 rounded w-20 h-8"),
			ctd.Container("bg-green-500 rounded w-20 h-8"),
			ctd.Container("bg-green-400 rounded w-20 h-8"),
			ctd.Container("bg-green-300 rounded w-20 h-8"),
			ctd.Container("bg-green-500 rounded w-20 h-8"),
			ctd.Container("bg-green-400 rounded w-20 h-8"),
		),

		// flex-wrap-reverse: items wrap in reverse order
		ctd.Text("flex-wrap-reverse (wrap, lines reversed)", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-wrap-reverse",
			ctd.Container("bg-purple-500 rounded w-20 h-8"),
			ctd.Container("bg-purple-400 rounded w-20 h-8"),
			ctd.Container("bg-purple-300 rounded w-20 h-8"),
			ctd.Container("bg-purple-500 rounded w-20 h-8"),
			ctd.Container("bg-purple-400 rounded w-20 h-8"),
			ctd.Container("bg-purple-300 rounded w-20 h-8"),
			ctd.Container("bg-purple-500 rounded w-20 h-8"),
			ctd.Container("bg-purple-400 rounded w-20 h-8"),
		),

		// flex-wrap with varying heights
		ctd.Text("flex-wrap with varying heights", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full flex-wrap items-start",
			ctd.Container("bg-orange-500 rounded w-16 h-8"),
			ctd.Container("bg-orange-400 rounded w-16 h-12"),
			ctd.Container("bg-orange-300 rounded w-16 h-6"),
			ctd.Container("bg-orange-500 rounded w-16 h-10"),
			ctd.Container("bg-orange-400 rounded w-16 h-8"),
			ctd.Container("bg-orange-300 rounded w-16 h-14"),
			ctd.Container("bg-orange-500 rounded w-16 h-8"),
			ctd.Container("bg-orange-400 rounded w-16 h-10"),
		),
	)
}

// buildFlexBasisSection demonstrates flex-basis classes
func buildFlexBasisSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text("Flex Basis", "text-yellow-400 text-lg"),
		ctd.Text("basis-0, basis-auto, basis-1/2, basis-full", "text-gray-400 text-xs"),

		// basis-0 with flex-grow: items share space equally regardless of content
		ctd.Text("basis-0 flex-grow (equal distribution)", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			ctd.VStack("bg-red-500 rounded p-2 basis-0 flex-grow",
				ctd.Text("Short", "text-white text-xs"),
			),
			ctd.VStack("bg-red-400 rounded p-2 basis-0 flex-grow",
				ctd.Text("Medium text", "text-white text-xs"),
			),
			ctd.VStack("bg-red-300 rounded p-2 basis-0 flex-grow",
				ctd.Text("Longer content", "text-white text-xs"),
			),
		),

		// basis-1/2: item starts at 50% width
		ctd.Text("basis-1/2 + flex-grow items", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			ctd.Container("bg-yellow-500 rounded basis-1/2 h-8"),
			ctd.Container("bg-yellow-400 rounded flex-grow h-8"),
			ctd.Container("bg-yellow-300 rounded flex-grow h-8"),
		),

		// basis-1/4: each item takes 1/4 width
		ctd.Text("basis-1/4 items", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 w-full",
			ctd.Container("bg-cyan-500 rounded basis-1/4 h-8"),
			ctd.Container("bg-cyan-400 rounded basis-1/4 h-8"),
			ctd.Container("bg-cyan-300 rounded basis-1/4 h-8"),
			ctd.Container("bg-cyan-200 rounded basis-1/4 h-8"),
		),
	)
}

// buildAlignSelfSection demonstrates align-self override
func buildAlignSelfSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text("Align Self", "text-yellow-400 text-lg"),
		ctd.Text("Parent: items-center | Children override", "text-gray-400 text-xs"),

		ctd.HStack("bg-gray-700 rounded gap-2 p-2 items-center h-24",
			ctd.VStack("bg-pink-500 rounded p-2 self-start",
				ctd.Text("self-start", "text-white text-xs"),
			),
			ctd.VStack("bg-pink-400 rounded p-2 self-center",
				ctd.Text("self-center", "text-white text-xs"),
			),
			ctd.VStack("bg-pink-300 rounded p-2 self-end",
				ctd.Text("self-end", "text-white text-xs"),
			),
			ctd.VStack("bg-pink-200 rounded p-2 self-stretch",
				ctd.Text("self-stretch", "text-gray-800 text-xs"),
			),
			ctd.VStack("bg-pink-100 rounded p-2",
				ctd.Text("(default)", "text-gray-800 text-xs"),
			),
		),
	)
}

// buildPositionSection demonstrates CSS positioning (relative, absolute, sticky)
func buildPositionSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-2",
		ctd.Text("Positioning", "text-yellow-400 text-lg"),

		// Relative positioning example
		ctd.Text("relative + top-2 left-4", "text-gray-400 text-xs"),
		ctd.HStack("bg-gray-700 rounded gap-2 p-2 h-12",
			ctd.Container("bg-blue-500 rounded w-10 h-6"),
			ctd.Container("bg-blue-400 rounded w-10 h-6 relative top-2 left-4"),
			ctd.Container("bg-blue-500 rounded w-10 h-6"),
		),

		// Absolute positioning example
		ctd.Text("absolute children in relative parent", "text-gray-400 text-xs"),
		ctd.ZStack("bg-gray-700 rounded p-2 h-16 w-full relative",
			ctd.Text("Content", "text-white text-xs"),
			ctd.VStack("bg-red-500 rounded px-1 absolute top-1 right-1",
				ctd.Text("TR", "text-white text-xs"),
			),
			ctd.VStack("bg-green-500 rounded px-1 absolute bottom-1 left-1",
				ctd.Text("BL", "text-white text-xs"),
			),
		),

		// Absolute inset (all edges)
		ctd.Text("absolute inset (all edges set)", "text-gray-400 text-xs"),
		ctd.ZStack("bg-gray-700 rounded h-12 w-full relative",
			ctd.Container("bg-purple-500 rounded absolute top-2 right-2 bottom-2 left-2"),
		),

		// Nested containing blocks
		ctd.Text("nested: absolute finds outer relative", "text-gray-400 text-xs"),
		ctd.ZStack("bg-gray-700 rounded p-2 h-20 w-full relative",
			ctd.Text("Outer", "text-white text-xs"),
			ctd.VStack("bg-gray-600 rounded p-2 w-3/4 h-10",
				ctd.Text("Inner (static)", "text-gray-300 text-xs"),
				ctd.VStack("bg-cyan-500 rounded px-1 absolute bottom-1 right-1",
					ctd.Text("In outer!", "text-white text-xs"),
				),
			),
		),

		// Sticky positioning
		ctd.Text("sticky header (scroll to test)", "text-gray-400 text-xs"),
		ctd.VStack("bg-gray-700 rounded h-16 w-full overflow-y-auto",
			ctd.Text("Sticky", "text-white text-xs bg-yellow-600 sticky top-0 px-2"),
			ctd.Text("Item 1", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 2", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 3", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 4", "text-gray-300 text-xs px-2"),
			ctd.Text("Item 5", "text-gray-300 text-xs px-2"),
		),
	)
}

// buildTextAlignSection demonstrates text alignment classes
func buildTextAlignSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text("Text Alignment", "text-yellow-400 text-lg"),
		ctd.Text("text-left, text-center, text-right, text-justify", "text-gray-400 text-xs"),

		// Basic alignments
		ctd.VStack("bg-gray-700 rounded p-2 gap-2 w-full",
			ctd.Text("Left aligned (default)", "text-white text-sm text-left w-full"),
			ctd.Text("Center aligned text", "text-white text-sm text-center w-full"),
			ctd.Text("Right aligned text", "text-white text-sm text-right w-full"),
		),

		// Responsive alignment
		ctd.Text("Responsive: text-left md:text-center lg:text-right", "text-gray-400 text-xs"),
		ctd.VStack("bg-gray-700 rounded p-2 w-full",
			ctd.Text("Resize window to see alignment change", "text-cyan-300 text-sm text-left md:text-center lg:text-right w-full"),
		),

		// start/end aliases
		ctd.Text("text-start / text-end (LTR aliases)", "text-gray-400 text-xs"),
		ctd.VStack("bg-gray-700 rounded p-2 gap-2 w-full",
			ctd.Text("text-start (same as left)", "text-white text-sm text-start w-full"),
			ctd.Text("text-end (same as right)", "text-white text-sm text-end w-full"),
		),

		// Justified text
		ctd.Text("text-justify (spreads words to fill width)", "text-gray-400 text-xs"),
		ctd.VStack("bg-gray-700 rounded p-2 w-full",
			ctd.Text("This is justified text that spans multiple lines. The words are spread out evenly so that each line fills the full width of the container, creating clean edges on both sides.", "text-white text-sm text-justify w-full"),
		),
	)
}

// buildOrderSection demonstrates order classes for visual reordering
func buildOrderSection() *ctd.Widget {
	return ctd.VStack("bg-gray-800 rounded-xl p-4 gap-3",
		ctd.Text("Order (visual reordering)", "text-yellow-400 text-lg"),
		ctd.Text("DOM order: A, B, C, D, E", "text-gray-400 text-xs"),
		ctd.Text("Visual: C(first), A(1), B(2), E(3), D(last)", "text-gray-400 text-xs"),

		ctd.HStack("bg-gray-700 rounded gap-2 p-2",
			// A: order-1
			ctd.VStack("bg-orange-500 rounded p-2 order-1",
				ctd.Text("A", "text-white text-lg"),
				ctd.Text("order-1", "text-white text-xs"),
			),
			// B: order-2
			ctd.VStack("bg-orange-400 rounded p-2 order-2",
				ctd.Text("B", "text-white text-lg"),
				ctd.Text("order-2", "text-white text-xs"),
			),
			// C: order-first
			ctd.VStack("bg-orange-600 rounded p-2 order-first",
				ctd.Text("C", "text-white text-lg"),
				ctd.Text("order-first", "text-white text-xs"),
			),
			// D: order-last
			ctd.VStack("bg-orange-300 rounded p-2 order-last",
				ctd.Text("D", "text-gray-800 text-lg"),
				ctd.Text("order-last", "text-gray-800 text-xs"),
			),
			// E: order-3
			ctd.VStack("bg-orange-200 rounded p-2 order-3",
				ctd.Text("E", "text-gray-800 text-lg"),
				ctd.Text("order-3", "text-gray-800 text-xs"),
			),
		),
	)
}
