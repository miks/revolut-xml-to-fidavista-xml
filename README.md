# revolut2fidavista

Converts Revolut CSV exports to FIDAVISTA XML format (used by Latvian banks).

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
2. **Drag a Revolut CSV** onto the app icon
3. The XML appears next to the CSV with the same name

That's it. No Python, no dependencies, nothing to install.

## CLI usage (for power users)

```bash
./revolut2fidavista transactions.csv
# → produces transactions.xml in the same folder

# Multiple files at once:
./revolut2fidavista jan.csv feb.csv mar.csv
```

## Notes

- Only `COMPLETED` transactions are included (pending/declined are skipped)
- Client name & account number are left blank — fill them in manually or
  add them as flags if you want to extend the tool
- The XML uses UTF-8 encoding
