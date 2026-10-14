-- Imports a pack through the packaged macOS app's real UI with Accessibility scripting
-- (WKWebView has no WebDriver). Usage: osascript <this file> <process name> <pack path>
-- Prints the install heading; errors with the visible text when the import fails.

on run argv
	set processName to item 1 of argv
	set packPath to item 2 of argv
	tell application "System Events"
		repeat 60 times
			if exists (window 1 of process processName) then exit repeat
			delay 1
		end repeat
		tell process processName
			set frontmost to true
			set appWindow to window 1
		end tell
	end tell
	my clickElement(my waitFor(appWindow, {"link", "button", "UI element"}, "Packs", 60))
	set pathField to my waitFor(appWindow, {"text field"}, "Local pack path", 60)
	tell application "System Events"
		set focused of pathField to true
		keystroke packPath
	end tell
	my clickElement(my waitFor(appWindow, {"button"}, "Review pack", 30))
	my clickElement(my waitFor(appWindow, {"button"}, "Install pack", 30))
	repeat 300 times
		set outcome to my installOutcome(appWindow)
		if outcome starts with "installed:" then return text 11 thru -1 of outcome
		if outcome starts with "failed:" then error (text 8 thru -1 of outcome) & linefeed & my visibleText(appWindow)
		delay 1
	end repeat
	error "timed out waiting for the install result" & linefeed & my visibleText(appWindow)
end run

on clickElement(element)
	tell application "System Events" to click element
end clickElement

on labelOf(element)
	tell application "System Events"
		repeat with attributeName in {"AXTitle", "AXDescription", "AXValue"}
			try
				set label to value of attribute attributeName of element
				if label is not missing value and (label as text) is not "" then return label as text
			end try
		end repeat
	end tell
	return ""
end labelOf

on waitFor(appWindow, classNames, label, timeoutSeconds)
	repeat timeoutSeconds times
		tell application "System Events" to set elements to entire contents of appWindow
		repeat with element in elements
			tell application "System Events" to set className to (class of element) as text
			if classNames contains className and my labelOf(element) is label then
				tell application "System Events"
					try
						if (value of attribute "AXEnabled" of element) is false then error "disabled"
						return contents of element
					end try
				end tell
			end if
		end repeat
		delay 1
	end repeat
	error "timed out waiting for " & label & linefeed & my visibleText(appWindow)
end waitFor

on installOutcome(appWindow)
	tell application "System Events" to set elements to entire contents of appWindow
	repeat with element in elements
		set label to my labelOf(element)
		if label ends with " is installed" or label ends with " is already installed" then return "installed:" & label
		if label ends with " was not installed" then return "failed:" & label
	end repeat
	return ""
end installOutcome

on visibleText(appWindow)
	set collected to ""
	tell application "System Events" to set elements to entire contents of appWindow
	repeat with element in elements
		set label to my labelOf(element)
		if label is not "" then set collected to collected & label & linefeed
	end repeat
	return collected
end visibleText
