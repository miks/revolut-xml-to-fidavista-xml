# revolut2fidavista

Converts Revolut **camt.053.001.12** (ISO 20022 Bank-to-Customer Statement) XML exports to FIDAVISTA XML format (used by Latvian banks).

## Build (on your Mac)

```bash
# Install Rust if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build & package
chmod +x build.sh
./build.sh
```

This produces `bundle/RevolutToFidavista.app`.

## For the accountant

1. Copy `RevolutToFidavista.app` to `/Applications` (or anywhere)
2. In Revolut Business, export a statement as XML (camt.053.001.12)
3. **Drag the XML file** onto the app icon
4. The FIDAVISTA XML appears next to the source file with a `.fidavista.xml` extension

That's it. No Python, no dependencies, nothing to install.

## CLI usage (for power users)

```bash
./revolut2fidavista statement.xml
# → produces statement.fidavista.xml in the same folder

# Multiple files at once:
./revolut2fidavista jan.xml feb.xml mar.xml
```

## If macOS says the app is "damaged"

This happens because the app isn't signed with an Apple certificate. Run this once in Terminal after copying the app:

```bash
xattr -cr /Applications/RevolutToFidavista.app
```

Then right-click → Open on first launch to get past the Gatekeeper prompt.

## Notes

- Opening and closing balances are taken from the `OPBD`/`CLBD` balance entries in the source XML
- Account IBAN is populated from the statement's account data
- The output XML uses UTF-8 encoding
