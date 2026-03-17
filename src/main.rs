use chrono::{DateTime, Local, NaiveDate, NaiveDateTime};
use csv::ReaderBuilder;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Debug, Deserialize)]
struct RevolutRow {
    #[serde(rename = "Type")]
    type_: String,
    #[serde(rename = "Started Date")]
    started_date: Option<String>,
    #[serde(rename = "Completed Date")]
    completed_date: Option<String>,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Amount")]
    amount: Option<f64>,
    #[serde(rename = "Currency")]
    currency: Option<String>,
    #[serde(rename = "State")]
    state: Option<String>,
    #[serde(rename = "Balance")]
    balance: Option<f64>,
    #[serde(rename = "Beneficiary IBAN")]
    beneficiary_iban: Option<String>,
    #[serde(rename = "Beneficiary BIC")]
    beneficiary_bic: Option<String>,
    #[serde(rename = "Reference")]
    reference: Option<String>,
    #[serde(rename = "ID")]
    id: Option<String>,
    // Ignore any extra columns
    #[serde(flatten, skip)]
    _extra: std::collections::HashMap<String, String>,
}

fn xe(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S") {
        return Some(dt.date());
    }
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

fn fmt_date(opt: &Option<String>, fallback: &str) -> String {
    opt.as_deref()
        .and_then(parse_date)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| fallback.to_string())
}

fn leaf(tag: &str, text: &str, indent: usize) -> String {
    format!("{}<{}>{}</{}>\n", "  ".repeat(indent), tag, xe(text), tag)
}

fn convert(csv_path: &Path) -> Result<PathBuf, String> {
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_path(csv_path)
        .map_err(|e| format!("Cannot open CSV: {}", e))?;

    let mut rows: Vec<RevolutRow> = Vec::new();
    for result in rdr.deserialize() {
        let row: RevolutRow = result.map_err(|e| format!("CSV parse error: {}", e))?;
        if let Some(ref s) = row.state {
            if s.to_uppercase() != "COMPLETED" {
                continue;
            }
        }
        rows.push(row);
    }

    if rows.is_empty() {
        return Err("No COMPLETED transactions found in CSV.".into());
    }

    let now: DateTime<Local> = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let timestamp = now.format("%Y%m%d%H%M%S000").to_string();

    let start_date = rows.iter()
        .filter_map(|r| r.completed_date.as_deref())
        .filter_map(parse_date)
        .min()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| today.clone());

    let end_date = rows.iter()
        .filter_map(|r| r.completed_date.as_deref())
        .filter_map(parse_date)
        .max()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| today.clone());

    let close_bal = rows.last().and_then(|r| r.balance).unwrap_or(0.0);
    let ccy = rows.iter().find_map(|r| r.currency.clone()).unwrap_or_else(|| "EUR".into());

    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<FIDAVISTA xmlns=\"http://www.bankasoc.lv/fidavista/fidavista0101.xsd\"\n");
    x.push_str("           xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n");

    // Header
    x.push_str("  <Header>\n");
    x.push_str(&leaf("Timestamp", &timestamp, 2));
    x.push_str(&leaf("From", "Revolut", 2));
    x.push_str("  </Header>\n");

    // Statement
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
    x.push_str(&leaf("AccNo", "", 3));
    x.push_str("      <CcyStmt>\n");
    x.push_str(&leaf("Ccy", &ccy, 4));
    x.push_str(&leaf("OpenBal", "0.00", 4));
    x.push_str(&leaf("CloseBal", &format!("{:.2}", close_bal), 4));

    for row in &rows {
        let is_topup = row.type_.to_uppercase() == "TOPUP";
        let amount = row.amount.unwrap_or(0.0);
        let amount_abs = format!("{:.2}", amount.abs());
        let book_date = fmt_date(&row.started_date, &today);
        let value_date = fmt_date(&row.completed_date, &today);
        let trans_id = row.id.as_deref().unwrap_or("");
        let description = row.description.as_deref().unwrap_or("");
        let reference = row.reference.as_deref().unwrap_or("");
        let pmt_info = if reference.is_empty() {
            description.to_string()
        } else {
            format!("{} {}", description, reference).trim().to_string()
        };
        let iban = row.beneficiary_iban.as_deref().unwrap_or("");
        let bic = row.beneficiary_bic.as_deref().unwrap_or("");
        let row_ccy = row.currency.as_deref().unwrap_or("EUR");

        x.push_str("        <TrxSet>\n");
        x.push_str(&leaf("TypeCode", if is_topup { "INB" } else { "OUT" }, 5));
        x.push_str(&leaf("TypeName", if is_topup { "INP" } else { "OUT" }, 5));
        x.push_str(&leaf("Type", if is_topup { "03" } else { "04" }, 5));
        x.push_str(&leaf("BookDate", &book_date, 5));
        x.push_str(&leaf("ValueDate", &value_date, 5));
        x.push_str(&leaf("BankRef", trans_id, 5));
        x.push_str(&leaf("DocNo", trans_id, 5));
        x.push_str(&leaf("CorD", if amount >= 0.0 { "C" } else { "D" }, 5));
        x.push_str(&leaf("AccAmt", &amount_abs, 5));
        x.push_str(&leaf("PmtInfo", if pmt_info.is_empty() { "No details" } else { &pmt_info }, 5));
        x.push_str("          <CPartySet>\n");
        x.push_str(&leaf("AccNo", iban, 6));
        x.push_str("            <AccHolder>\n");
        x.push_str(&leaf("Name", if is_topup { "" } else { description }, 7));
        x.push_str(&leaf("LegalId", "", 7));
        x.push_str("            </AccHolder>\n");
        x.push_str(&leaf("BankCode", bic, 6));
        x.push_str(&leaf("Ccy", row_ccy, 6));
        x.push_str(&leaf("Amt", &amount_abs, 6));
        x.push_str("          </CPartySet>\n");
        x.push_str("        </TrxSet>\n");
    }

    x.push_str("      </CcyStmt>\n");
    x.push_str("    </AccountSet>\n");
    x.push_str("  </Statement>\n");
    x.push_str("</FIDAVISTA>\n");

    // Output path: same name, .xml extension
    let out_path = csv_path.with_extension("xml");
    fs::write(&out_path, x).map_err(|e| format!("Cannot write XML: {}", e))?;

    Ok(out_path)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("revolut2fidavista — Revolut CSV → FIDAVISTA XML converter");
        eprintln!();
        eprintln!("Usage:  revolut2fidavista <file.csv> [file2.csv ...]");
        eprintln!();
        eprintln!("Tip: on macOS you can drag a CSV onto this binary in Terminal.");
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
