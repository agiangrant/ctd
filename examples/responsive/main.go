// Responsive Design Example
// Demonstrates responsive breakpoints with Tailwind-style sm:, md:, lg:, xl:, 2xl: prefixes.
// Resize the window to see styles change at different breakpoints.
package main

import (
	"fmt"
	"log"
	"runtime"

	"github.com/agiangrant/centered/internal/ffi"
	"github.com/agiangrant/centered/retained"
	"github.com/agiangrant/centered/tw"
)

func init() {
	runtime.LockOSThread()
}

// UI holds references to widgets that need responsive updates
type UI struct {
	root           *retained.Widget
	header         *retained.Widget
	statusText     *retained.Widget
	cards          []*retained.Widget
	lastBreakpoint tw.Breakpoint
}

func main() {
	config := retained.DefaultLoopConfig()
	loop := retained.NewLoop(config)
	tree := loop.Tree()

	ui := buildUI()
	tree.SetRoot(ui.root)

	loop.OnResize(func(width, height float32) {
		ui.root.SetSize(width, height)

		// Update responsive styles when window resizes
		bp := loop.Breakpoints()
		currentBP := bp.ActiveBreakpoint(width)

		// Update status text to show current breakpoint
		bpName := breakpointName(currentBP)
		ui.statusText.SetText(fmt.Sprintf("Width: %.0fpx | Breakpoint: %s", width, bpName))

		// Only re-apply styles if breakpoint changed
		if currentBP != ui.lastBreakpoint {
			ui.lastBreakpoint = currentBP

			// Re-apply responsive styles to all widgets
			ui.header.UpdateStateForWidth(tw.StateDefault, width, bp)
			ui.statusText.UpdateStateForWidth(tw.StateDefault, width, bp)
			for _, card := range ui.cards {
				card.UpdateStateForWidth(tw.StateDefault, width, bp)
			}
		}
	})

	loop.OnEvent(func(event ffi.Event) bool {
		if event.Type == ffi.EventKeyPressed && event.Keycode() == uint32(ffi.KeyEscape) {
			ffi.RequestExit()
			return true
		}
		return false
	})

	appConfig := ffi.DefaultAppConfig()
	appConfig.Title = "Responsive Design Demo"
	appConfig.Width = 1000
	appConfig.Height = 700

	log.Println("Starting Responsive Design Demo...")
	log.Println("  - Resize the window to see styles change")
	log.Println("  - Breakpoints: sm: ≥640px, md: ≥768px, lg: ≥1024px, xl: ≥1280px, 2xl: ≥1536px")
	log.Println("  - Press ESC to quit")

	if err := loop.Run(appConfig); err != nil {
		log.Fatal(err)
	}
}

func buildUI() *UI {
	ui := &UI{
		lastBreakpoint: tw.BreakpointBase,
	}

	// Root container
	ui.root = retained.Container("bg-gray-900").
		WithSize(1000, 700)

	// Header - changes text size at breakpoints
	// Base: text-lg, md: text-xl, lg: text-2xl, xl: text-3xl
	ui.header = retained.Text("Responsive Design Demo", "text-white text-lg md:text-xl lg:text-2xl xl:text-3xl").
		WithFrame(20, 20, 960, 40)

	// Status bar - shows current breakpoint
	// Changes color at breakpoints: base gray, sm blue, md green, lg yellow, xl orange, 2xl red
	ui.statusText = retained.Text("Width: 1000px | Breakpoint: lg", "text-gray-400 sm:text-blue-400 md:text-green-400 lg:text-yellow-400 xl:text-orange-400 text-sm").
		WithFrame(20, 65, 600, 20)

	// Breakpoint indicator cards
	breakpoints := []struct {
		name      string
		threshold string
		classes   string
	}{
		{"Base", "< 640px", "bg-gray-700 text-gray-400"},
		{"SM", "≥ 640px", "bg-gray-700 sm:bg-blue-600 text-gray-400 sm:text-white"},
		{"MD", "≥ 768px", "bg-gray-700 md:bg-green-600 text-gray-400 md:text-white"},
		{"LG", "≥ 1024px", "bg-gray-700 lg:bg-yellow-600 text-gray-400 lg:text-black"},
		{"XL", "≥ 1280px", "bg-gray-700 xl:bg-orange-600 text-gray-400 xl:text-white"},
		{"2XL", "≥ 1536px", "bg-gray-700 text-gray-400"}, // 2xl prefix not yet parsed
	}

	cardWidth := float32(150)
	cardHeight := float32(80)
	cardGap := float32(15)
	startX := float32(20)
	startY := float32(110)

	for i, bp := range breakpoints {
		x := startX + float32(i)*(cardWidth+cardGap)

		// Card container
		card := retained.Container(bp.classes+" rounded-lg p-3").
			WithFrame(x, startY, cardWidth, cardHeight)

		// Card title
		title := retained.Text(bp.name, "text-white text-lg font-bold").
			WithFrame(x+12, startY+12, 80, 24)

		// Card threshold
		threshold := retained.Text(bp.threshold, "text-gray-300 text-xs").
			WithFrame(x+12, startY+42, 120, 16)

		card.WithChildren(title, threshold)
		ui.cards = append(ui.cards, card)
	}

	// Info panel
	infoPanel := retained.Container("bg-gray-800 rounded-xl p-4").
		WithFrame(20, 220, 960, 460)

	infoTitle := retained.Text("How It Works", "text-yellow-400 text-lg").
		WithFrame(36, 236, 200, 24)

	info := []string{
		"1. Tailwind classes support breakpoint prefixes: sm:, md:, lg:, xl:, 2xl:",
		"2. Styles are mobile-first: base applies always, breakpoints layer on top",
		"3. Example: 'text-sm md:text-lg' = small on mobile, large on md and up",
		"4. The Loop tracks window width and provides ActiveBreakpoint()",
		"5. Call UpdateStateForWidth() on widgets when resize triggers breakpoint change",
	}

	var infoWidgets []*retained.Widget
	infoWidgets = append(infoWidgets, infoTitle)

	yOffset := float32(275)
	for _, line := range info {
		text := retained.Text(line, "text-gray-300 text-sm").
			WithFrame(36, yOffset, 920, 20)
		infoWidgets = append(infoWidgets, text)
		yOffset += 24
	}

	// Supported classes section
	supportedTitle := retained.Text("Supported Responsive Properties", "text-green-400 text-lg").
		WithFrame(36, yOffset+20, 300, 24)
	infoWidgets = append(infoWidgets, supportedTitle)
	yOffset += 50

	categories := []struct {
		name  string
		items string
	}{
		{"Colors", "text-*, bg-*, border-*"},
		{"Typography", "text-{xs,sm,base,lg,xl,...}, font-{thin,normal,bold,...}"},
		{"Spacing", "p-*, px-*, py-*, m-*, mx-*, my-*, gap-*"},
		{"Sizing", "w-*, h-*, min-w-*, max-w-*"},
		{"Layout", "flex, grid, hidden, block, flex-col, flex-row"},
		{"Borders", "rounded-*, border-*"},
	}

	for _, cat := range categories {
		nameText := retained.Text(cat.name+":", "text-blue-400 text-xs").
			WithFrame(36, yOffset, 100, 16)
		itemsText := retained.Text(cat.items, "text-gray-400 text-xs").
			WithFrame(140, yOffset, 800, 16)
		infoWidgets = append(infoWidgets, nameText, itemsText)
		yOffset += 20
	}

	infoPanel.WithChildren(infoWidgets...)

	// Build the full widget tree
	children := []*retained.Widget{ui.header, ui.statusText}
	children = append(children, ui.cards...)
	children = append(children, infoPanel)
	ui.root.WithChildren(children...)

	return ui
}

func breakpointName(bp tw.Breakpoint) string {
	switch bp {
	case tw.BreakpointSM:
		return "sm (≥640px)"
	case tw.BreakpointMD:
		return "md (≥768px)"
	case tw.BreakpointLG:
		return "lg (≥1024px)"
	case tw.BreakpointXL:
		return "xl (≥1280px)"
	case tw.Breakpoint2XL:
		return "2xl (≥1536px)"
	default:
		return "base (<640px)"
	}
}
