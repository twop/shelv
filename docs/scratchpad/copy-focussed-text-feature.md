### Research on "copy focussed feature"
- Options: Use accessibility or emulate cmd + c
- Research:
  * https://stackoverflow.com/questions/19798583/accessibility-api-alternative-to-get-selected-text-from-any-app-in-osx
  * https://stackoverflow.com/questions/76009610/get-selected-text-when-in-any-application-on-macos
  * https://stackoverflow.com/questions/1487175/how-to-obtain-the-selected-text-from-the-frontmost-application-in-macos
  * https://www.alfredapp.com/blog/tips-and-tricks/manipulating-selected-text-in-macos-with-alfred-workflows/
- Alfred like raycast has this capability
- Can we do that through accessiblity API?
  * if yes, does it work in a browser?
- can we access clipboard and still be on app store?
  * cmd + c -> global hotkey to dump clipboard into shelv without opening
  * if we emulate cmd + c can we publish on app store?

### Thoughts on Rust/ObjC/Swift architecture
- Make a reasonable attempt to get this feature working w/ Rust <-> ObjC
- Swift discussion is worth having -> but not right now 
	* Approaches:
		* Crux https://github.com/redbadger/crux (I/O and UI is swift, with Rust lib)
		* Swift app = host, get window and use egui (I/O can be split between swift/rust, but UI is still rust)
			* Publishing is easier, Rust can still do as much as we want it to do (host layer is non-trivial though)
		* Swift lib = can use Swift for better OS level implementations
		* Do nothing -> embrace Rust safety (longer we don't have to choose, the better)
- Simon: Really wants to make this feature hackable
	* Good first attempt at a good hackable API
	* Fantastic demo for the pitch of hackability
	* This makes the feature easier implement right now
- Data we have:
	* Selected text
	* URL
	* Document
	* Application
	* Window title

## Update 10/03/2024
- We realized that even the A11Y API is not allowed for sandboxed apps, so we're pausing this feature now.
- Current branch is here: https://github.com/briskmode/shelv/tree/copy-focussed
