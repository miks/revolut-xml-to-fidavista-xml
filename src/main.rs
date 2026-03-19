use chrono::{DateTime, Local, NaiveDate};
use roxmltree::Document;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

const NS: &str = "urn:iso:std:iso:20022:tech:xsd:camt.053.001.12";

fn xe(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn leaf(tag: &str, text: &str, indent: usize) -> String {
    format!("{}<{}>{}</{}>\n", "  ".repeat(indent), tag, xe(text), tag)
}

/// Find first child element with the given local name in our namespace.
fn child<'a>(node: roxmltree::Node<'a, 'a>, local: &str) -> Option<roxmltree::Node<'a, 'a>> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == local && n.tag_name().namespace() == Some(NS))
}

/// Get trimmed text of first matching child.
fn child_text<'a>(node: roxmltree::Node<'a, 'a>, local: &str) -> Option<String> {
    child(node, local).and_then(|n| n.text()).map(|t| t.trim().to_string())
}

/// Descend through a chain of tag names and return text of the final node.
fn descend_text<'a>(node: roxmltree::Node<'a, 'a>, path: &[&str]) -> Option<String> {
    let mut cur = node;
    for &seg in path {
        cur = child(cur, seg)?;
    }
    cur.text().map(|t| t.trim().to_string())
}

/// Parse ISO 8601 datetime string down to a YYYY-MM-DD date string.
fn dt_to_date(s: &str) -> Option<String> {
    // Try full datetime first, then date-only
    if let Ok(dt) = DateTime::parse_from_rfc3339(s.trim()) {
        return Some(dt.format("%Y-%m-%d").to_string());
    }
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .ok()
        .map(|d| d.format("%Y-%m-%d").to_string())
}

struct Entry {
    amount: String,
    ccy: String,
    is_credit: bool,
    book_date: String,
    value_date: String,
    ref_id: String,
    pmt_info: String,
    counterparty_name: String,
    counterparty_iban: String,
    fmly_cd: String,
}

fn convert(xml_path: &Path) -> Result<PathBuf, String> {
    let src = fs::read_to_string(xml_path).map_err(|e| format!("Cannot read file: {}", e))?;
    let doc = Document::parse(&src).map_err(|e| format!("XML parse error: {}", e))?;

    let root = doc.root_element();

    let root_ns = root.tag_name().namespace().unwrap_or("");
    if root_ns != NS {
        return Err(format!(
            "Unsupported XML format.\nExpected namespace: {}\nGot:               {}",
            NS,
            if root_ns.is_empty() { "(none)" } else { root_ns }
        ));
    }

    let stmt = [&["BkToCstmrStmt", "Stmt"]]
        .iter()
        .find_map(|path| {
            let mut n = root;
            for &seg in path.iter() {
                n = child(n, seg)?;
            }
            Some(n)
        })
        .ok_or("Cannot find BkToCstmrStmt/Stmt")?;

    // Account info
    let iban = descend_text(stmt, &["Acct", "Id", "IBAN"]).unwrap_or_default();
    let ccy = descend_text(stmt, &["Acct", "Ccy"]).unwrap_or_else(|| "EUR".into());

    // Period
    let fr_to = child(stmt, "FrToDt").ok_or("Missing FrToDt")?;
    let start_date = child_text(fr_to, "FrDtTm")
        .and_then(|s| dt_to_date(&s))
        .unwrap_or_default();
    let end_date = child_text(fr_to, "ToDtTm")
        .and_then(|s| dt_to_date(&s))
        .unwrap_or_default();

    // Balances
    let mut open_bal = "0.00".to_string();
    let mut close_bal = "0.00".to_string();
    for bal in stmt.children().filter(|n| n.is_element() && n.tag_name().name() == "Bal" && n.tag_name().namespace() == Some(NS)) {
        let cd = descend_text(bal, &["Tp", "CdOrPrtry", "Cd"]).unwrap_or_default();
        let amt = child(bal, "Amt").and_then(|n| n.text()).map(|t| t.trim().to_string()).unwrap_or_default();
        let is_dbt = child_text(bal, "CdtDbtInd").as_deref() == Some("DBIT");
        let signed = if is_dbt { format!("-{}", amt) } else { amt };
        match cd.as_str() {
            "OPBD" => open_bal = signed,
            "CLBD" => close_bal = signed,
            _ => {}
        }
    }

    // Transactions
    let mut entries: Vec<Entry> = Vec::new();
    for ntry in stmt.children().filter(|n| n.is_element() && n.tag_name().name() == "Ntry" && n.tag_name().namespace() == Some(NS)) {
        let amt_node = child(ntry, "Amt").ok_or("Ntry missing Amt")?;
        let amount = amt_node.text().unwrap_or("0").trim().to_string();
        let entry_ccy = amt_node.attribute("Ccy").unwrap_or(&ccy).to_string();
        let cdt_dbt = child_text(ntry, "CdtDbtInd").unwrap_or_default();
        let is_credit = cdt_dbt == "CRDT";

        let book_date = descend_text(ntry, &["BookgDt", "DtTm"])
            .and_then(|s| dt_to_date(&s))
            .unwrap_or_default();
        let value_date = descend_text(ntry, &["ValDt", "DtTm"])
            .and_then(|s| dt_to_date(&s))
            .unwrap_or_default();

        let ref_id = child_text(ntry, "AcctSvcrRef").unwrap_or_default();

        let fmly_cd = descend_text(ntry, &["BkTxCd", "Domn", "Fmly", "Cd"]).unwrap_or_default();

        // Drill into NtryDtls/TxDtls
        let tx_dtls = child(ntry, "NtryDtls").and_then(|n| child(n, "TxDtls"));

        let pmt_info = tx_dtls
            .and_then(|td| descend_text(td, &["RmtInf", "Ustrd"]))
            .unwrap_or_default();

        let rltd = tx_dtls.and_then(|td| child(td, "RltdPties"));

        // Counterparty: prefer Dbtr (for credits), otherwise InitgPty (for debits)
        let counterparty_name = rltd
            .and_then(|r| {
                descend_text(r, &["Dbtr", "Pty", "Nm"])
                    .or_else(|| descend_text(r, &["InitgPty", "Pty", "Nm"]))
                    .or_else(|| descend_text(r, &["Cdtr", "Pty", "Nm"]))
            })
            .unwrap_or_default();

        let counterparty_iban = rltd
            .and_then(|r| {
                descend_text(r, &["DbtrAcct", "Id", "IBAN"])
                    .or_else(|| descend_text(r, &["CdtrAcct", "Id", "IBAN"]))
            })
            .unwrap_or_default();

        entries.push(Entry {
            amount,
            ccy: entry_ccy,
            is_credit,
            book_date,
            value_date,
            ref_id,
            pmt_info,
            counterparty_name,
            counterparty_iban,
            fmly_cd,
        });
    }

    if entries.is_empty() {
        return Err("No transactions found in XML.".into());
    }

    let now: DateTime<Local> = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let timestamp = now.format("%Y%m%d%H%M%S000").to_string();

    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<FIDAVISTA xmlns=\"http://www.bankasoc.lv/fidavista/fidavista0101.xsd\"\n");
    x.push_str("           xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n");

    x.push_str("  <Header>\n");
    x.push_str(&leaf("Timestamp", &timestamp, 2));
    x.push_str(&leaf("From", "Revolut", 2));
    x.push_str("  </Header>\n");

    x.push_str("  <Statement>\n");
    x.push_str("    <Period>\n");
    x.push_str(&leaf("StartDate", &start_date, 3));
    x.push_str(&leaf("EndDate", &end_date, 3));
    x.push_str(&leaf("PrepDate", &today, 3));
    x.push_str("    </Period>\n");

    x.push_str("    <BankSet>\n");
    x.push_str(&leaf("Name", "Revolut", 3));
    x.push_str(&leaf("LegalId", "", 3));
    x.push_str(&leaf("Address", "", 3));
    x.push_str("    </BankSet>\n");

    x.push_str("    <ClientSet>\n");
    x.push_str(&leaf("Name", "", 3));
    x.push_str(&leaf("LegalId", "", 3));
    x.push_str("    </ClientSet>\n");

    x.push_str("    <AccountSet>\n");
    x.push_str(&leaf("AccNo", &iban, 3));
    x.push_str("      <CcyStmt>\n");
    x.push_str(&leaf("Ccy", &ccy, 4));
    x.push_str(&leaf("OpenBal", &open_bal, 4));
    x.push_str(&leaf("CloseBal", &close_bal, 4));

    for e in &entries {
        // RCDT family = incoming credit transfer → INB; else OUT
        let is_inb = e.fmly_cd == "RCDT" || e.is_credit;
        let type_code = if is_inb { "INB" } else { "OUT" };
        let type_name = if is_inb { "INP" } else { "OUT" };
        let type_num = if is_inb { "03" } else { "04" };
        let cor_d = if e.is_credit { "C" } else { "D" };
        let desc = if e.pmt_info.is_empty() { "No details" } else { &e.pmt_info };

        x.push_str("        <TrxSet>\n");
        x.push_str(&leaf("TypeCode", type_code, 5));
        x.push_str(&leaf("TypeName", type_name, 5));
        x.push_str(&leaf("Type", type_num, 5));
        x.push_str(&leaf("BookDate", &e.book_date, 5));
        x.push_str(&leaf("ValueDate", &e.value_date, 5));
        x.push_str(&leaf("BankRef", &e.ref_id, 5));
        x.push_str(&leaf("DocNo", &e.ref_id, 5));
        x.push_str(&leaf("CorD", cor_d, 5));
        x.push_str(&leaf("AccAmt", &e.amount, 5));
        x.push_str(&leaf("PmtInfo", desc, 5));
        x.push_str("          <CPartySet>\n");
        x.push_str(&leaf("AccNo", &e.counterparty_iban, 6));
        x.push_str("            <AccHolder>\n");
        x.push_str(&leaf("Name", &e.counterparty_name, 7));
        x.push_str(&leaf("LegalId", "", 7));
        x.push_str("            </AccHolder>\n");
        x.push_str(&leaf("BankCode", "", 6));
        x.push_str(&leaf("Ccy", &e.ccy, 6));
        x.push_str(&leaf("Amt", &e.amount, 6));
        x.push_str("          </CPartySet>\n");
        x.push_str("        </TrxSet>\n");
    }

    x.push_str("      </CcyStmt>\n");
    x.push_str("    </AccountSet>\n");
    x.push_str("  </Statement>\n");
    x.push_str("</FIDAVISTA>\n");

    let out_path = xml_path.with_extension("fidavista.xml");
    fs::write(&out_path, x).map_err(|e| format!("Cannot write XML: {}", e))?;

    Ok(out_path)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("revolut2fidavista — Revolut camt.053.001.12 XML → FIDAVISTA XML converter");
        eprintln!();
        eprintln!("Usage:  revolut2fidavista <file.xml> [file2.xml ...]");
        eprintln!();
        eprintln!("Tip: on macOS you can drag an XML onto this binary in Terminal.");
        process::exit(1);
    }

    let mut any_error = false;
    for arg in &args[1..] {
        let path = Path::new(arg);
        match convert(path) {
            Ok(out) => println!("✓  {} → {}", path.display(), out.display()),
            Err(e) => {
                eprintln!("✗  {}: {}", path.display(), e);
                any_error = true;
            }
        }
    }

    if any_error {
        process::exit(1);
    }
}
