// Comprehensive Flexbox Layout Example
// Demonstrates all flexbox features: direction, justify-content, align-items, flex-grow/shrink, and gap
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
	appConfig.Title = "Flexbox Layout Demo"
	appConfig.Width = 1280
	appConfig.Height = 900

	log.Println("Starting Flexbox Layout Demo...")
	log.Println("  - flex-direction: row, column, row-reverse, column-reverse")
	log.Println("  - justify-content: start, end, center, between, around, evenly")
	log.Println("  - align-items: start, end, center, stretch")
	log.Println("  - flex-grow and flex-shrink")
	log.Println("  - gap spacing")
	log.Println("Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI() *ctd.Widget {
	root := ctd.Container("bg-gray-900").
		WithSize(1280, 900)

	// Header
	header := ctd.Text("Flexbox Layout Comprehensive Demo", "text-white text-2xl").
		WithFrame(20, 20, 600, 30)

	// ============ SECTION 1: Flex Direction (x=20, y=70) ============
	directionSection := buildDirectionSection()

	// ============ SECTION 2: Justify Content (x=20, y=320) ============
	justifySection := buildJustifySection()

	// ============ SECTION 3: Align Items (x=660, y=70) ============
	alignSection := buildAlignSection()

	// ============ SECTION 4: Flex Grow/Shrink (x=660, y=400) ============
	growSection := buildGrowSection()

	// ============ SECTION 5: Nested Flex (x=20, y=580) ============
	nestedSection := buildNestedSection()

	// ============ SECTION 6: Real-World Example (x=660, y=580) ============
	realWorldSection := buildRealWorldSection()

	root.WithChildren(
		header,
		directionSection,
		justifySection,
		alignSection,
		growSection,
		nestedSection,
		realWorldSection,
	)

	return root
}

// buildDirectionSection demonstrates flex-direction: row, column, row-reverse, column-reverse
func buildDirectionSection() *ctd.Widget {
	section := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(20, 70, 620, 230)

	title := ctd.Text("flex-direction", "text-yellow-400 text-lg").
		WithFrame(36, 86, 200, 24)

	// Row
	rowLabel := ctd.Text("flex-row", "text-gray-400 text-xs").
		WithFrame(36, 120, 80, 16)
	rowContainer := ctd.Container("bg-gray-700 rounded flex flex-row gap-2 p-2").
		WithFrame(36, 140, 260, 50)
	for i := 0; i < 4; i++ {
		colors := []string{"bg-red-500", "bg-yellow-500", "bg-green-500", "bg-blue-500"}
		box := ctd.Container(colors[i] + " rounded").WithSize(40, 30)
		rowContainer.AddChild(box)
	}

	// Column
	colLabel := ctd.Text("flex-col", "text-gray-400 text-xs").
		WithFrame(320, 120, 80, 16)
	colContainer := ctd.Container("bg-gray-700 rounded flex flex-col gap-2 p-2").
		WithFrame(320, 140, 80, 140)
	for i := 0; i < 3; i++ {
		colors := []string{"bg-purple-500", "bg-pink-500", "bg-indigo-500"}
		box := ctd.Container(colors[i] + " rounded").WithSize(60, 30)
		colContainer.AddChild(box)
	}

	// Row Reverse
	rowRevLabel := ctd.Text("flex-row-reverse", "text-gray-400 text-xs").
		WithFrame(36, 200, 120, 16)
	rowRevContainer := ctd.Container("bg-gray-700 rounded flex flex-row-reverse gap-2 p-2").
		WithFrame(36, 220, 260, 50)
	for i := 0; i < 4; i++ {
		labels := []string{"1", "2", "3", "4"}
		box := ctd.VStack("bg-cyan-500 rounded p-1",
			ctd.Text(labels[i], "text-white text-sm"),
		).WithSize(40, 30)
		rowRevContainer.AddChild(box)
	}

	// Column Reverse
	colRevLabel := ctd.Text("flex-col-reverse", "text-gray-400 text-xs").
		WithFrame(420, 120, 120, 16)
	colRevContainer := ctd.Container("bg-gray-700 rounded flex flex-col-reverse gap-2 p-2").
		WithFrame(420, 140, 80, 140)
	for i := 0; i < 3; i++ {
		labels := []string{"A", "B", "C"}
		box := ctd.VStack("bg-orange-500 rounded p-1",
			ctd.Text(labels[i], "text-white text-sm"),
		).WithSize(60, 30)
		colRevContainer.AddChild(box)
	}

	// Code hint
	codeHint := ctd.Text("Classes: flex flex-row | flex-col | flex-row-reverse | flex-col-reverse", "text-gray-500 text-xs").
		WithFrame(36, 280, 580, 16)

	section.WithChildren(
		title,
		rowLabel, rowContainer,
		colLabel, colContainer,
		rowRevLabel, rowRevContainer,
		colRevLabel, colRevContainer,
		codeHint,
	)

	return section
}

// buildJustifySection demonstrates all justify-content values
func buildJustifySection() *ctd.Widget {
	section := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(20, 320, 620, 240)

	title := ctd.Text("justify-content (main axis alignment)", "text-yellow-400 text-lg").
		WithFrame(36, 336, 400, 24)

	justifyValues := []struct {
		name  string
		class string
	}{
		{"justify-start", "flex flex-row justify-start gap-2 p-2"},
		{"justify-end", "flex flex-row justify-end gap-2 p-2"},
		{"justify-center", "flex flex-row justify-center gap-2 p-2"},
		{"justify-between", "flex flex-row justify-between p-2"},
		{"justify-around", "flex flex-row justify-around p-2"},
		{"justify-evenly", "flex flex-row justify-evenly p-2"},
	}

	var children []*ctd.Widget
	children = append(children, title)

	yOffset := float32(366)
	for _, jv := range justifyValues {
		label := ctd.Text(jv.name, "text-gray-400 text-xs").
			WithFrame(36, yOffset, 100, 14)

		container := ctd.Container("bg-gray-700 rounded " + jv.class).
			WithFrame(140, yOffset-2, 480, 30)

		for i := 0; i < 3; i++ {
			box := ctd.Container("bg-blue-500 rounded").WithSize(40, 20)
			container.AddChild(box)
		}

		children = append(children, label, container)
		yOffset += 34
	}

	section.WithChildren(children...)
	return section
}

// buildAlignSection demonstrates all align-items values
func buildAlignSection() *ctd.Widget {
	section := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(660, 70, 600, 310)

	title := ctd.Text("align-items (cross axis alignment)", "text-yellow-400 text-lg").
		WithFrame(676, 86, 400, 24)

	alignValues := []struct {
		name  string
		class string
	}{
		{"items-start", "flex flex-row items-start gap-2 p-2"},
		{"items-end", "flex flex-row items-end gap-2 p-2"},
		{"items-center", "flex flex-row items-center gap-2 p-2"},
		{"items-stretch", "flex flex-row items-stretch gap-2 p-2"},
	}

	var children []*ctd.Widget
	children = append(children, title)

	yOffset := float32(120)
	for _, av := range alignValues {
		label := ctd.Text(av.name, "text-gray-400 text-xs").
			WithFrame(676, yOffset, 100, 14)

		container := ctd.Container("bg-gray-700 rounded " + av.class).
			WithFrame(676, yOffset+16, 560, 50)

		// Different height boxes to show alignment
		heights := []float32{20, 35, 25, 40}
		colors := []string{"bg-green-500", "bg-green-400", "bg-green-600", "bg-green-300"}
		for i := 0; i < 4; i++ {
			box := ctd.Container(colors[i] + " rounded").WithSize(60, heights[i])
			container.AddChild(box)
		}

		children = append(children, label, container)
		yOffset += 70
	}

	// Code hint
	codeHint := ctd.Text("Classes: items-start | items-end | items-center | items-stretch", "text-gray-500 text-xs").
		WithFrame(676, 355, 560, 16)
	children = append(children, codeHint)

	section.WithChildren(children...)
	return section
}

// buildGrowSection demonstrates flex-grow and flex-shrink
func buildGrowSection() *ctd.Widget {
	section := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(660, 400, 600, 160)

	title := ctd.Text("flex-grow / flex-shrink", "text-yellow-400 text-lg").
		WithFrame(676, 416, 300, 24)

	// flex-grow demo
	growLabel := ctd.Text("flex-grow (middle item grows)", "text-gray-400 text-xs").
		WithFrame(676, 450, 200, 14)
	growContainer := ctd.Container("bg-gray-700 rounded flex flex-row gap-2 p-2").
		WithFrame(676, 468, 560, 40)

	// First box - no grow
	box1 := ctd.Container("bg-purple-500 rounded").WithSize(60, 30)
	// Middle box - flex-grow
	box2 := ctd.Container("bg-purple-400 rounded flex-grow").WithSize(60, 30)
	// Last box - no grow
	box3 := ctd.Container("bg-purple-500 rounded").WithSize(60, 30)

	growContainer.WithChildren(box1, box2, box3)

	// Multiple growers
	multiGrowLabel := ctd.Text("Multiple flex-grow items share space", "text-gray-400 text-xs").
		WithFrame(676, 516, 300, 14)
	multiGrowContainer := ctd.Container("bg-gray-700 rounded flex flex-row gap-2 p-2").
		WithFrame(676, 534, 560, 40)

	multiBox1 := ctd.Container("bg-pink-500 rounded flex-grow").WithSize(40, 30)
	multiBox2 := ctd.Container("bg-pink-400 rounded flex-grow").WithSize(40, 30)
	multiBox3 := ctd.Container("bg-pink-500 rounded flex-grow").WithSize(40, 30)

	multiGrowContainer.WithChildren(multiBox1, multiBox2, multiBox3)

	section.WithChildren(
		title,
		growLabel, growContainer,
		multiGrowLabel, multiGrowContainer,
	)

	return section
}

// buildNestedSection demonstrates nested flex containers
func buildNestedSection() *ctd.Widget {
	section := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(20, 580, 620, 300)

	title := ctd.Text("Nested Flex Containers", "text-yellow-400 text-lg").
		WithFrame(36, 596, 300, 24)

	subtitle := ctd.Text("Outer: flex-row, Inner: flex-col", "text-gray-400 text-xs").
		WithFrame(36, 624, 300, 14)

	// Outer container with flex-row
	outer := ctd.Container("bg-gray-700 rounded flex flex-row gap-4 p-4").
		WithFrame(36, 644, 580, 220)

	// First inner column
	inner1 := ctd.Container("bg-gray-600 rounded flex flex-col gap-2 p-2 flex-grow").
		WithSize(0, 180)
	inner1.AddChild(ctd.Text("Column 1", "text-white text-sm").WithSize(100, 20))
	for i := 0; i < 3; i++ {
		inner1.AddChild(ctd.Container("bg-red-500 rounded").WithSize(100, 40))
	}

	// Second inner column
	inner2 := ctd.Container("bg-gray-600 rounded flex flex-col gap-2 p-2 flex-grow").
		WithSize(0, 180)
	inner2.AddChild(ctd.Text("Column 2", "text-white text-sm").WithSize(100, 20))
	for i := 0; i < 4; i++ {
		inner2.AddChild(ctd.Container("bg-yellow-500 rounded").WithSize(100, 30))
	}

	// Third inner column with nested row
	inner3 := ctd.Container("bg-gray-600 rounded flex flex-col gap-2 p-2 flex-grow").
		WithSize(0, 180)
	inner3.AddChild(ctd.Text("Column 3 (nested row)", "text-white text-sm").WithSize(150, 20))

	nestedRow := ctd.Container("bg-gray-500 rounded flex flex-row gap-1 p-1").
		WithSize(130, 40)
	for i := 0; i < 3; i++ {
		nestedRow.AddChild(ctd.Container("bg-green-500 rounded").WithSize(35, 30))
	}
	inner3.AddChild(nestedRow)
	inner3.AddChild(ctd.Container("bg-blue-500 rounded").WithSize(130, 60))

	outer.WithChildren(inner1, inner2, inner3)

	section.WithChildren(title, subtitle, outer)
	return section
}

// buildRealWorldSection shows a realistic UI layout using flexbox
func buildRealWorldSection() *ctd.Widget {
	section := ctd.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(660, 580, 600, 300)

	title := ctd.Text("Real-World Example: Card Layout", "text-yellow-400 text-lg").
		WithFrame(676, 596, 400, 24)

	// Card container - horizontal card layout
	cardRow := ctd.Container("bg-gray-700 rounded flex flex-row gap-4 p-4").
		WithFrame(676, 630, 560, 240)

	// Card 1
	card1 := buildCard("Dashboard", "View analytics and metrics", "bg-blue-600", "bg-blue-500")

	// Card 2
	card2 := buildCard("Settings", "Configure your preferences", "bg-green-600", "bg-green-500")

	// Card 3
	card3 := buildCard("Profile", "Manage your account", "bg-purple-600", "bg-purple-500")

	cardRow.WithChildren(card1, card2, card3)

	section.WithChildren(title, cardRow)
	return section
}

// buildCard creates a sample card with flexbox layout
func buildCard(titleText, descText, headerColor, iconColor string) *ctd.Widget {
	card := ctd.Container("bg-gray-800 rounded-lg flex flex-col flex-grow").
		WithSize(0, 200)

	// Card header
	header := ctd.Container(headerColor + " rounded-t-lg flex flex-row items-center gap-2 p-3").
		WithSize(0, 50)
	icon := ctd.Container(iconColor + " rounded").WithSize(24, 24)
	headerTitle := ctd.Text(titleText, "text-white text-sm").WithSize(100, 20)
	header.WithChildren(icon, headerTitle)

	// Card body
	body := ctd.Container("flex flex-col gap-2 p-3 flex-grow").
		WithSize(0, 0)
	desc := ctd.Text(descText, "text-gray-400 text-xs").WithSize(140, 30)
	body.AddChild(desc)

	// Card footer with button
	footer := ctd.Container("flex flex-row justify-end p-2").
		WithSize(0, 40)
	button := ctd.Container("bg-gray-600 rounded px-3 py-1").
		WithSize(60, 28)
	buttonText := ctd.Text("Open", "text-white text-xs").WithSize(40, 16)
	button.AddChild(buttonText)
	footer.AddChild(button)

	card.WithChildren(header, body, footer)
	return card
}
