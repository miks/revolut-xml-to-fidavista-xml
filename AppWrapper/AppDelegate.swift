import Cocoa

class AppDelegate: NSObject, NSApplicationDelegate {

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Launched without files (double-clicked directly) — show usage hint
    }

    // Called when files are dropped onto the dock icon or app icon in Finder
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        var results: [String] = []
        var hasError = false

        for filename in filenames {
            guard filename.lowercased().hasSuffix(".xml") else {
                results.append("⚠️ Skipped (not an XML): \(filename)")
                continue
            }

            let binaryURL = Bundle.main.bundleURL
                .appendingPathComponent("Contents/MacOS/revolut2fidavista")

            let process = Process()
            process.executableURL = binaryURL
            process.arguments = [filename]

            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = pipe

            do {
                try process.run()
                process.waitUntilExit()
                let output = String(
                    data: pipe.fileHandleForReading.readDataToEndOfFile(),
                    encoding: .utf8
                ) ?? ""

                if process.terminationStatus == 0 {
                    let outPath = (filename as NSString).deletingPathExtension + ".fidavista.xml"
                    results.append("✓ \((outPath as NSString).lastPathComponent)")
                } else {
                    results.append("✗ \((filename as NSString).lastPathComponent): \(output.trimmingCharacters(in: .whitespacesAndNewlines))")
                    hasError = true
                }
            } catch {
                results.append("✗ Could not run converter: \(error.localizedDescription)")
                hasError = true
            }
        }

        let message = results.joined(separator: "\n")
        let alert = NSAlert()
        alert.messageText = hasError ? "Conversion finished with errors" : "Conversion successful"
        alert.informativeText = message
        alert.alertStyle = hasError ? .warning : .informational
        alert.addButton(withTitle: "OK")
        alert.runModal()

        NSApp.terminate(nil)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}

// Explicit entry point — required when compiling a single file without -parse-as-library
let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
