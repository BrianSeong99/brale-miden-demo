//! Table formatting helpers for demo output.
//!
//! Hand-rolled Unicode box-drawing tables — no external dependencies.

/// Print a balance table showing issuer supply and wallet holdings.
///
/// ```text
/// ┌──────────┬─────────┬─────────┐
/// │ Account  │ USDB    │ Role    │
/// ├──────────┼─────────┼─────────┤
/// │ Issuer   │ S: 1000 │ Faucet  │
/// │ Wallet A │ 1000    │ Holder  │
/// │ Wallet B │ 0       │ Holder  │
/// └──────────┴─────────┴─────────┘
/// ```
pub fn print_balance_table(
    token_symbol: &str,
    issuer_supply: u64,
    wallet_a_balance: u64,
    wallet_b_balance: u64,
) {
    let supply_str = format!("S: {issuer_supply}");
    let a_str = wallet_a_balance.to_string();
    let b_str = wallet_b_balance.to_string();

    // Column widths: Account(10), Token(9), Role(8)
    let col_acct = 10;
    let col_token = token_symbol.len().max(supply_str.len()).max(a_str.len()).max(b_str.len()).max(7) + 2;
    let col_role = 8;

    let hdr_token = format!("{token_symbol:<width$}", width = col_token);
    let hdr_role = format!("{:<width$}", "Role", width = col_role);

    let sep_top = format!(
        "┌{:─<col_acct$}┬{:─<col_token$}┬{:─<col_role$}┐",
        "", "", ""
    );
    let sep_mid = format!(
        "├{:─<col_acct$}┼{:─<col_token$}┼{:─<col_role$}┤",
        "", "", ""
    );
    let sep_bot = format!(
        "└{:─<col_acct$}┴{:─<col_token$}┴{:─<col_role$}┘",
        "", "", ""
    );

    println!("{sep_top}");
    println!(
        "│ {:<width_a$}│ {hdr_token}│ {hdr_role}│",
        "Account",
        width_a = col_acct - 1
    );
    println!("{sep_mid}");
    println!(
        "│ {:<width_a$}│ {:<width_t$}│ {:<width_r$}│",
        "Issuer",
        supply_str,
        "Faucet",
        width_a = col_acct - 1,
        width_t = col_token - 1,
        width_r = col_role - 1
    );
    println!(
        "│ {:<width_a$}│ {:<width_t$}│ {:<width_r$}│",
        "Wallet A",
        a_str,
        "Holder",
        width_a = col_acct - 1,
        width_t = col_token - 1,
        width_r = col_role - 1
    );
    println!(
        "│ {:<width_a$}│ {:<width_t$}│ {:<width_r$}│",
        "Wallet B",
        b_str,
        "Holder",
        width_a = col_acct - 1,
        width_t = col_token - 1,
        width_r = col_role - 1
    );
    println!("{sep_bot}");
}

/// Print a signer status table with checkmark/pending indicators.
///
/// ```text
/// ┌──────────┬────────────┬──────────────┐
/// │ Signer   │ Status     │ Commitment   │
/// ├──────────┼────────────┼──────────────┤
/// │ Signer 1 │ ✓ Signed   │ 0x4eac…0324 │
/// │ Signer 2 │ ✓ Signed   │ 0xea7e…2af4 │
/// │ Signer 3 │ — Pending  │ 0x2015…bc16 │
/// └──────────┴────────────┴──────────────┘
/// Threshold: 2/3 met ✓
/// ```
pub fn print_signer_table(signers: &[(&str, &str, bool)], threshold: u32) {
    let col_name = 10;
    let col_status = 12;
    let col_commit = 14;

    let sep_top = format!(
        "┌{:─<col_name$}┬{:─<col_status$}┬{:─<col_commit$}┐",
        "", "", ""
    );
    let sep_mid = format!(
        "├{:─<col_name$}┼{:─<col_status$}┼{:─<col_commit$}┤",
        "", "", ""
    );
    let sep_bot = format!(
        "└{:─<col_name$}┴{:─<col_status$}┴{:─<col_commit$}┘",
        "", "", ""
    );

    println!("{sep_top}");
    println!(
        "│ {:<width_n$}│ {:<width_s$}│ {:<width_c$}│",
        "Signer",
        "Status",
        "Commitment",
        width_n = col_name - 1,
        width_s = col_status - 1,
        width_c = col_commit - 1
    );
    println!("{sep_mid}");

    let mut signed_count = 0u32;
    for (name, commitment_hex, signed) in signers {
        let status = if *signed {
            signed_count += 1;
            "✓ Signed"
        } else {
            "— Pending"
        };

        // Truncate commitment: "0x4eac…0324" (first 6 + … + last 4)
        let commit_display = if commitment_hex.len() > 12 {
            format!(
                "{}…{}",
                &commitment_hex[..6],
                &commitment_hex[commitment_hex.len() - 4..]
            )
        } else {
            commitment_hex.to_string()
        };

        println!(
            "│ {:<width_n$}│ {:<width_s$}│ {:<width_c$}│",
            name,
            status,
            commit_display,
            width_n = col_name - 1,
            width_s = col_status - 1,
            width_c = col_commit - 1
        );
    }
    println!("{sep_bot}");

    let total = signers.len() as u32;
    if signed_count >= threshold {
        println!("Threshold: {signed_count}/{total} met ✓");
    } else {
        println!("Threshold: {signed_count}/{total} (need {threshold})");
    }
}
