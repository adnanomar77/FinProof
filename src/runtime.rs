use crate::ast::{AccountType, Declaration, Expression, Program, Statement};
use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    pub account_type: AccountType,
    pub currency: String,
    pub balance: crate::types::MoneyAmount,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionState {
    pub accounts: HashMap<String, AccountState>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub sequence: u64,
    pub logical_time: u64,
    pub transaction_sequence: u64,
    pub transaction: String,
    pub operation: String,
    pub amount: crate::types::MoneyAmount,
    pub currency: String,
    pub from: String,
    pub to: String,
    pub from_before: crate::types::MoneyAmount,
    pub from_after: crate::types::MoneyAmount,
    pub to_before: crate::types::MoneyAmount,
    pub to_after: crate::types::MoneyAmount,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionResult {
    pub state: ExecutionState,
    pub ledger: Vec<LedgerEntry>,
    pub trace: Vec<crate::verification::ExecutionTraceEntry>,
}
impl LedgerEntry {
    #[allow(dead_code)]
    pub fn canonical_representation(&self) -> String {
        format!(
            "sequence={};logical_time={};transaction_sequence={};transaction={};operation={};amount_minor_units={};currency={};from={};to={};from_before={};from_after={};to_before={};to_after={}",
            self.sequence,
            self.logical_time,
            self.transaction_sequence,
            self.transaction,
            self.operation,
            self.amount.minor_units(),
            self.currency,
            self.from,
            self.to,
            self.from_before.minor_units(),
            self.from_after.minor_units(),
            self.to_before.minor_units(),
            self.to_after.minor_units(),
        )
    }
}
impl ExecutionState {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }
}
pub(crate) fn validate_execution_invariants(
    initial_state: &ExecutionState,
    state: &ExecutionState,
    ledger: &[LedgerEntry],
) -> Result<(), String> {
    for (account_name, account) in &state.accounts {
        if account.balance.minor_units() < 0 {
            return Err(format!(
                "FP2010 INVARIANT_VIOLATION: account '{}' has a negative balance.",
                account_name
            ));
        }
    }
    for (index, entry) in ledger.iter().enumerate() {
        let expected_sequence = (index + 1) as u64;
        if index == 0 {
            if entry.transaction_sequence != 1 {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: first ledger entry must have transaction sequence 1, found {}.",
                    entry.transaction_sequence
                ));
            }
        } else {
            let previous = &ledger[index - 1];
            if entry.transaction_sequence < previous.transaction_sequence
                || entry.transaction_sequence > previous.transaction_sequence + 1
            {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} has invalid transaction sequence {}; previous entry has {}.",
                    entry.sequence, entry.transaction_sequence, previous.transaction_sequence
                ));
            }
        }
        if index > 0 {
            let previous = &ledger[index - 1];
            if entry.transaction_sequence == previous.transaction_sequence
                && entry.transaction != previous.transaction
            {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} changes transaction name within transaction sequence {}.",
                    entry.sequence, entry.transaction_sequence
                ));
            }
        }
        if entry.sequence != expected_sequence {
            return Err(format!(
                "FP2010 INVARIANT_VIOLATION: ledger sequence expected {}, found {}.",
                expected_sequence, entry.sequence
            ));
        }
        if entry.logical_time != entry.sequence {
            return Err(format!(
                "FP2010 INVARIANT_VIOLATION: ledger entry {} has logical time {}, expected {}.",
                entry.sequence, entry.logical_time, entry.sequence
            ));
        }
        if entry.amount.minor_units() <= 0 {
            return Err(format!(
                "FP2010 INVARIANT_VIOLATION: ledger entry {} has a non-positive amount.",
                entry.sequence
            ));
        }
        match entry.operation.as_str() {
            "pay" | "transfer" => {
                let from_account = state.accounts.get(&entry.from).ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: ledger entry {} references unknown source account '{}'.",                        entry.sequence, entry.from                    )                })?;
                let to_account = state.accounts.get(&entry.to).ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: ledger entry {} references unknown destination account '{}'.",                        entry.sequence, entry.to                    )                })?;
                if from_account.currency != entry.currency {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: ledger entry {} currency does not match source account '{}'.",
                        entry.sequence, entry.from
                    ));
                }
                if to_account.currency != entry.currency {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: ledger entry {} currency does not match destination account '{}'.",
                        entry.sequence, entry.to
                    ));
                }
                let expected_from_after = entry.from_before.checked_sub(entry.amount).map_err(|_| {                    format!(                        "FP2010 INVARIANT_VIOLATION: ledger entry {} source transition overflows.",                        entry.sequence                    )                })?;
                let expected_to_after = entry.to_before.checked_add(entry.amount).map_err(|_| {                    format!(                        "FP2010 INVARIANT_VIOLATION: ledger entry {} destination transition overflows.",                        entry.sequence                    )                })?;
                if entry.from_after != expected_from_after {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: ledger entry {} has an invalid source balance transition.",
                        entry.sequence
                    ));
                }
                if entry.to_after != expected_to_after {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: ledger entry {} has an invalid destination balance transition.",
                        entry.sequence
                    ));
                }
            }
            "debit" | "credit" => {
                let account_name = if entry.operation == "debit" {
                    &entry.from
                } else {
                    &entry.to
                };
                let account = state.accounts.get(account_name).ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: ledger entry {} references unknown account '{}'.",                        entry.sequence, account_name                    )                })?;
                if account.currency != entry.currency {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: ledger entry {} currency does not match account '{}'.",
                        entry.sequence, account_name
                    ));
                }
                let before = if entry.operation == "debit" {
                    entry.from_before
                } else {
                    entry.to_before
                };
                let after = if entry.operation == "debit" {
                    entry.from_after
                } else {
                    entry.to_after
                };
                let increases = match account.account_type {
                    AccountType::Asset | AccountType::Expense => entry.operation == "debit",
                    AccountType::Liability | AccountType::Equity | AccountType::Revenue => {
                        entry.operation == "credit"
                    }
                };
                let expected_after = if increases {
                    before.checked_add(entry.amount).map_err(|_| {                        format!(                            "FP2010 INVARIANT_VIOLATION: ledger entry {} balance transition overflows.",                            entry.sequence                        )                    })?
                } else {
                    before.checked_sub(entry.amount).map_err(|_| {                        format!(                            "FP2010 INVARIANT_VIOLATION: ledger entry {} balance transition underflows.",                            entry.sequence                        )                    })?
                };
                if after != expected_after {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: ledger entry {} has an invalid {} balance transition.",
                        entry.sequence, entry.operation
                    ));
                }
                if entry.operation == "debit"
                    && (!entry.to.is_empty()
                        || entry.to_before.minor_units() != 0
                        || entry.to_after.minor_units() != 0)
                {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: debit ledger entry {} has invalid destination fields.",
                        entry.sequence
                    ));
                }
                if entry.operation == "credit"
                    && (!entry.from.is_empty()
                        || entry.from_before.minor_units() != 0
                        || entry.from_after.minor_units() != 0)
                {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: credit ledger entry {} has invalid source fields.",
                        entry.sequence
                    ));
                }
            }
            operation => {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} has unknown operation '{}'.",
                    entry.sequence, operation
                ));
            }
        }
    }
    let mut account_first_seen: HashMap<(String, String), bool> = HashMap::new();
    for entry in ledger {
        let first_seen_accounts = match entry.operation.as_str() {
            "pay" | "transfer" => vec![
                (&entry.from, entry.from_before),
                (&entry.to, entry.to_before),
            ],
            "debit" => vec![(&entry.from, entry.from_before)],
            "credit" => vec![(&entry.to, entry.to_before)],
            _ => Vec::new(),
        };
        for (account_name, before_balance) in first_seen_accounts {
            let key = (account_name.clone(), entry.currency.clone());
            if let std::collections::hash_map::Entry::Vacant(e) = account_first_seen.entry(key) {
                let initial_account = initial_state.accounts.get(account_name).ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: ledger entry {} references account '{}' missing from initial state.",                        entry.sequence, account_name                    )                })?;
                if initial_account.currency != entry.currency {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: account '{}' initial currency {} does not match ledger currency {}.",
                        account_name, initial_account.currency, entry.currency
                    ));
                }
                if initial_account.balance != before_balance {
                    return Err(format!(
                        "FP2010 INVARIANT_VIOLATION: account '{}' first ledger balance is {}, but initial balance is {}.",
                        account_name, before_balance, initial_account.balance
                    ));
                }
                e.insert(true);
            }
        }
    }
    let mut account_last_balance: HashMap<(String, String), crate::types::MoneyAmount> =
        HashMap::new();
    for entry in ledger {
        if entry.operation == "pay" || entry.operation == "transfer" {
            let from_key = (entry.from.clone(), entry.currency.clone());
            if let Some(last_balance) = account_last_balance.get(&from_key)
                && *last_balance != entry.from_before
            {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} source account '{}' does not continue from its previous balance.",
                    entry.sequence, entry.from
                ));
            }
            account_last_balance.insert(from_key, entry.from_after);
            let to_key = (entry.to.clone(), entry.currency.clone());
            if let Some(last_balance) = account_last_balance.get(&to_key)
                && *last_balance != entry.to_before
            {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} destination account '{}' does not continue from its previous balance.",
                    entry.sequence, entry.to
                ));
            }
            account_last_balance.insert(to_key, entry.to_after);
        } else if entry.operation == "debit" {
            let key = (entry.from.clone(), entry.currency.clone());
            if let Some(last_balance) = account_last_balance.get(&key)
                && *last_balance != entry.from_before
            {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} debit account '{}' does not continue from its previous balance.",
                    entry.sequence, entry.from
                ));
            }
            account_last_balance.insert(key, entry.from_after);
        } else if entry.operation == "credit" {
            let key = (entry.to.clone(), entry.currency.clone());
            if let Some(last_balance) = account_last_balance.get(&key)
                && *last_balance != entry.to_before
            {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: ledger entry {} credit account '{}' does not continue from its previous balance.",
                    entry.sequence, entry.to
                ));
            }
            account_last_balance.insert(key, entry.to_after);
        }
    }
    for (account_name, account) in &state.accounts {
        let key = (account_name.clone(), account.currency.clone());
        if let Some(last_balance) = account_last_balance.get(&key) {
            if account.balance != *last_balance {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: account '{}' final balance is {}, but ledger final balance is {}.",
                    account_name, account.balance, last_balance
                ));
            }
        } else {
            let initial_account = initial_state.accounts.get(account_name).ok_or_else(|| {                format!(                    "FP2010 INVARIANT_VIOLATION: account '{}' exists in final state but not in initial state.",                    account_name                )            })?;
            if account.balance != initial_account.balance {
                return Err(format!(
                    "FP2010 INVARIANT_VIOLATION: account '{}' changed from {} to {} without a ledger entry.",
                    account_name, initial_account.balance, account.balance
                ));
            }
        }
    }
    let mut conservation: HashMap<(u64, String), i64> = HashMap::new();
    for entry in ledger {
        if entry.operation == "pay" || entry.operation == "transfer" {
            let key = (entry.transaction_sequence, entry.currency.clone());
            let from_delta = entry.from_after.minor_units()                .checked_sub(entry.from_before.minor_units())                .ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: transaction '{}' currency '{}' conservation calculation overflows.",                        entry.transaction, entry.currency                    )                })?;
            let to_delta = entry.to_after.minor_units()                .checked_sub(entry.to_before.minor_units())                .ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: transaction '{}' currency '{}' conservation calculation overflows.",                        entry.transaction, entry.currency                    )                })?;
            let net_delta = from_delta.checked_add(to_delta).ok_or_else(|| {                format!(                    "FP2010 INVARIANT_VIOLATION: transaction '{}' currency '{}' conservation calculation overflows.",                    entry.transaction, entry.currency                )            })?;
            let total = conservation.entry(key).or_insert(0);
            *total = total.checked_add(net_delta).ok_or_else(|| {                format!(                    "FP2010 INVARIANT_VIOLATION: transaction '{}' currency '{}' conservation total overflows.",                    entry.transaction, entry.currency                )            })?;
        }
    }
    for ((transaction_sequence, currency), net_delta) in conservation {
        if net_delta != 0 {
            return Err(format!(
                "FP2010 INVARIANT_VIOLATION: transaction '{}' does not conserve {} value; net delta is {} minor units.",
                transaction_sequence, currency, net_delta
            ));
        }
    }
    let mut double_entry_totals: HashMap<(u64, String), (i64, i64)> = HashMap::new();
    for entry in ledger {
        if entry.operation == "debit" || entry.operation == "credit" {
            let key = (entry.transaction_sequence, entry.currency.clone());
            let totals = double_entry_totals.entry(key).or_insert((0, 0));
            if entry.operation == "debit" {
                totals.0 = totals.0.checked_add(entry.amount.minor_units()).ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: transaction '{}' currency '{}' debit total overflows.",                        entry.transaction, entry.currency                    )                })?;
            } else {
                totals.1 = totals.1.checked_add(entry.amount.minor_units()).ok_or_else(|| {                    format!(                        "FP2010 INVARIANT_VIOLATION: transaction '{}' currency '{}' credit total overflows.",                        entry.transaction, entry.currency                    )                })?;
            }
        }
    }
    for ((transaction_sequence, currency), (debit_total, credit_total)) in double_entry_totals {
        if debit_total != credit_total {
            return Err(format!(
                "FP2010 INVARIANT_VIOLATION: transaction '{}' is unbalanced in {}; debit total is {}, credit total is {} minor units.",
                transaction_sequence, currency, debit_total, credit_total
            ));
        }
    }
    Ok(())
}
#[allow(dead_code)]
pub fn execute(program: &Program) -> Result<ExecutionResult, String> {
    let mut state = ExecutionState::new();
    let mut ledger = Vec::new();
    let mut transaction_sequence = 0u64;
    for declaration in &program.declarations {
        if let Declaration::Account(account) = declaration {
            if state.accounts.contains_key(&account.name) {
                return Err(format!(
                    "FP2002 DUPLICATE_ACCOUNT: account '{}' already exists.",
                    account.name
                ));
            }
            if account.initial_balance.minor_units() < 0 {
                return Err(format!(
                    "FP2008 INVALID_INITIAL_BALANCE: account '{}' cannot have a negative initial balance.",
                    account.name
                ));
            }
            state.accounts.insert(
                account.name.clone(),
                AccountState {
                    account_type: account.account_type.clone(),
                    currency: account.currency.clone(),
                    balance: crate::types::MoneyAmount::from_decimal_str(
                        &account.initial_balance.to_string(),
                    )
                    .map_err(|error| format!("FP2007 INVALID_AMOUNT: {}.", error))?,
                },
            );
        }
    }
    let initial_state = state.clone();
    for declaration in &program.declarations {
        if let Declaration::Transaction(transaction) = declaration {
            transaction_sequence += 1;
            let mut transaction_state = state.clone();
            let mut transaction_ledger = ledger.clone();
            for statement in &transaction.statements {
                match statement {
                    Statement::Pay(payment) => execute_move(
                        &mut transaction_state,
                        &mut transaction_ledger,
                        &MoveOperation {
                            transaction_name: &transaction.name,
                            transaction_sequence,
                            expression: &payment.amount,
                            from: &payment.from,
                            to: &payment.to,
                            operation: "pay",
                        },
                    )?,
                    Statement::Transfer(transfer) => execute_move(
                        &mut transaction_state,
                        &mut transaction_ledger,
                        &MoveOperation {
                            transaction_name: &transaction.name,
                            transaction_sequence,
                            expression: &transfer.amount,
                            from: &transfer.from,
                            to: &transfer.to,
                            operation: "transfer",
                        },
                    )?,
                    Statement::Debit(debit) => execute_double_entry(
                        &mut transaction_state,
                        &mut transaction_ledger,
                        &transaction.name,
                        transaction_sequence,
                        &debit.account,
                        &debit.amount,
                        true,
                    )?,
                    Statement::Credit(credit) => execute_double_entry(
                        &mut transaction_state,
                        &mut transaction_ledger,
                        &transaction.name,
                        transaction_sequence,
                        &credit.account,
                        &credit.amount,
                        false,
                    )?,
                }
            }
            validate_execution_invariants(&initial_state, &transaction_state, &transaction_ledger)?;
            state = transaction_state;
            ledger = transaction_ledger;
        }
    }
    Ok(ExecutionResult {
        state,
        ledger,
        trace: Vec::new(),
    })
}
fn execute_double_entry(
    state: &mut ExecutionState,
    ledger: &mut Vec<LedgerEntry>,
    transaction_name: &str,
    transaction_sequence: u64,
    account_name: &str,
    expression: &Expression,
    is_debit: bool,
) -> Result<(), String> {
    let amount = evaluate_expression(expression)?;
    let expression_currency = expression_currency(expression)?;
    if amount.minor_units() <= 0 {
        return Err(
            "FP2003 INVALID_AMOUNT: double-entry amount must be greater than zero.".to_string(),
        );
    }
    let account = state.accounts.get(account_name).ok_or_else(|| {
        format!(
            "FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.",
            account_name
        )
    })?;
    let account_currency = account.currency.clone();
    let account_type = account.account_type.clone();
    let before = account.balance;
    if expression_currency != account_currency {
        return Err(format!(
            "FP2005 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
            expression_currency, account_currency
        ));
    }
    let increases = match account_type {
        AccountType::Asset | AccountType::Expense => is_debit,
        AccountType::Liability | AccountType::Equity | AccountType::Revenue => !is_debit,
    };
    let after = if increases {
        before.checked_add(amount).map_err(|_| {            format!(                "FP2009 ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",                account_name            )        })?
    } else {
        if before.minor_units() < amount.minor_units() {
            return Err(format!(
                "FP2006 INSUFFICIENT_FUNDS: account '{}' has {}, but {} is required.",
                account_name,
                format_amount(before),
                format_amount(amount)
            ));
        }
        before.checked_sub(amount).map_err(|_| {            format!(                "FP2009 ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",                account_name            )        })?
    };
    let zero = crate::types::MoneyAmount::from_minor_units(0);
    state.accounts.get_mut(account_name).unwrap().balance = after;
    let sequence = u64::try_from(ledger.len())
        .map_err(|_| "FP2009 ARITHMETIC_OVERFLOW: ledger sequence overflow.".to_string())?
        .checked_add(1)
        .ok_or_else(|| "FP2009 ARITHMETIC_OVERFLOW: ledger sequence overflow.".to_string())?;
    ledger.push(LedgerEntry {
        sequence,
        logical_time: sequence,
        transaction_sequence,
        transaction: transaction_name.to_string(),
        operation: if is_debit {
            "debit".to_string()
        } else {
            "credit".to_string()
        },
        amount,
        currency: account_currency,
        from: if is_debit {
            account_name.to_string()
        } else {
            String::new()
        },
        to: if is_debit {
            String::new()
        } else {
            account_name.to_string()
        },
        from_before: if is_debit { before } else { zero },
        from_after: if is_debit { after } else { zero },
        to_before: if is_debit { zero } else { before },
        to_after: if is_debit { zero } else { after },
    });
    Ok(())
}
struct MoveOperation<'a> {
    transaction_name: &'a str,
    transaction_sequence: u64,
    expression: &'a Expression,
    from: &'a str,
    to: &'a str,
    operation: &'a str,
}
fn execute_move(
    state: &mut ExecutionState,
    ledger: &mut Vec<LedgerEntry>,
    operation: &MoveOperation<'_>,
) -> Result<(), String> {
    let amount = evaluate_expression(operation.expression)?;
    let expression_currency = expression_currency(operation.expression)?;
    if amount.minor_units() <= 0 {
        return Err(format!(
            "FP2003 INVALID_AMOUNT: {} amount must be greater than zero.",
            operation.operation
        ));
    }
    if operation.from == operation.to {
        return Err(format!(
            "FP2004 SELF_TRANSFER: account '{}' cannot be both source and destination.",
            operation.from
        ));
    }
    let from_account = state.accounts.get(operation.from).ok_or_else(|| {
        format!(
            "FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.",
            operation.from
        )
    })?;
    let from_currency = from_account.currency.clone();
    let from_balance = from_account.balance;
    let to_account = state.accounts.get(operation.to).ok_or_else(|| {
        format!(
            "FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.",
            operation.to
        )
    })?;
    let to_currency = to_account.currency.clone();
    let to_balance = to_account.balance;
    if from_currency != to_currency {
        return Err(format!(
            "FP2005 CURRENCY_MISMATCH: cannot move funds from {} to {}.",
            from_currency, to_currency
        ));
    }
    if expression_currency != from_currency {
        return Err(format!(
            "FP2005 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
            expression_currency, from_currency
        ));
    }
    if from_balance < amount {
        return Err(format!(
            "FP2006 INSUFFICIENT_FUNDS: account '{}' has {}, but {} is required.",
            operation.from,
            format_amount(from_balance),
            format_amount(amount)
        ));
    }
    let from_after = from_balance.checked_sub(amount).map_err(|_| {        format!("FP2009 ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.", operation.from)    })?;
    let to_after = to_balance.checked_add(amount).map_err(|_| {        format!("FP2009 ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.", operation.to)    })?;
    state.accounts.get_mut(operation.from).unwrap().balance = from_after;
    state.accounts.get_mut(operation.to).unwrap().balance = to_after;
    let sequence = u64::try_from(ledger.len())
        .map_err(|_| "FP2009 ARITHMETIC_OVERFLOW: ledger sequence overflow.".to_string())?
        .checked_add(1)
        .ok_or_else(|| "FP2009 ARITHMETIC_OVERFLOW: ledger sequence overflow.".to_string())?;
    ledger.push(LedgerEntry {
        sequence,
        logical_time: sequence,
        transaction_sequence: operation.transaction_sequence,
        transaction: operation.transaction_name.to_string(),
        operation: operation.operation.to_string(),
        amount,
        currency: from_currency,
        from: operation.from.to_string(),
        to: operation.to.to_string(),
        from_before: from_balance,
        from_after,
        to_before: to_balance,
        to_after,
    });
    Ok(())
}
fn evaluate_money_expression(expression: &Expression) -> Result<crate::types::MoneyAmount, String> {
    match expression {
        Expression::Money(money) => crate::types::MoneyAmount::from_decimal_str(&money.amount)
            .map_err(|error| format!("FP2007 INVALID_AMOUNT: {}.", error)),
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let left_value = evaluate_money_expression(left)?;
            let right_value = evaluate_money_expression(right)?;
            match operator {                crate::ast::BinaryOperator::Add => left_value.checked_add(right_value).map_err(|_| "FP2009 ARITHMETIC_OVERFLOW: monetary expression addition exceeds the supported range.".to_string()),                crate::ast::BinaryOperator::Subtract => {                    left_value.checked_sub(right_value).map_err(|_| "FP2009 ARITHMETIC_OVERFLOW: monetary expression subtraction exceeds the supported range.".to_string())                }            }
        }
    }
}
fn evaluate_expression(expression: &Expression) -> Result<crate::types::MoneyAmount, String> {
    evaluate_money_expression(expression)
}
fn expression_currency(expression: &Expression) -> Result<String, String> {
    match expression {
        Expression::Money(money) => Ok(money.currency.clone()),
        Expression::Binary { left, right, .. } => {
            let left_currency = expression_currency(left)?;
            let right_currency = expression_currency(right)?;
            if left_currency != right_currency {
                return Err(format!(
                    "FP2005 CURRENCY_MISMATCH: expression currencies {} and {} do not match.",
                    left_currency, right_currency
                ));
            }
            Ok(left_currency)
        }
    }
}
fn format_amount(value: crate::types::MoneyAmount) -> String {
    value.to_string()
}
#[cfg(test)]
mod ledger_canonical_tests {
    use super::*;
    #[test]
    fn ledger_entry_canonical_representation_is_deterministic() {
        let entry = LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("500.00").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("400.00").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0.00").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("100.00").unwrap(),
        };
        let first = entry.canonical_representation();
        let second = entry.canonical_representation();
        assert_eq!(first, second);
    }
}
#[cfg(test)]
mod runtime_invariant_tests {
    use super::*;
    #[test]
    fn runtime_invariant_rejects_non_positive_ledger_amount() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_minor_units(0),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
        }];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("non-positive amount"));
    }
    #[test]
    fn runtime_invariant_rejects_ledger_currency_mismatch() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            currency: "EUR".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
        }];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("currency does not match source account")
        );
    }
    #[test]
    fn runtime_invariant_rejects_invalid_ledger_transition() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("450").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
        }];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("invalid source balance transition")
        );
    }
    #[test]
    fn runtime_invariant_rejects_invalid_debit_credit_transition() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Cash".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                    },
                ),
                (
                    "RevenueAccount".to_string(),
                    AccountState {
                        account_type: AccountType::Revenue,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "debit".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: "Cash".to_string(),
                to: String::new(),
                from_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                to_before: crate::types::MoneyAmount::from_minor_units(0),
                to_after: crate::types::MoneyAmount::from_minor_units(0),
            },
            LedgerEntry {
                sequence: 2,
                logical_time: 2,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "credit".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: String::new(),
                to: "RevenueAccount".to_string(),
                from_before: crate::types::MoneyAmount::from_minor_units(0),
                from_after: crate::types::MoneyAmount::from_minor_units(0),
                to_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("140").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("invalid credit balance transition")
        );
    }
    #[test]
    fn runtime_invariant_rejects_unbalanced_double_entry() {
        let initial_state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Cash".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
                (
                    "RevenueAccount".to_string(),
                    AccountState {
                        account_type: AccountType::Revenue,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
            ]),
        };
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Cash".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                    },
                ),
                (
                    "RevenueAccount".to_string(),
                    AccountState {
                        account_type: AccountType::Revenue,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("140").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "debit".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: "Cash".to_string(),
                to: String::new(),
                from_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                to_before: crate::types::MoneyAmount::from_minor_units(0),
                to_after: crate::types::MoneyAmount::from_minor_units(0),
            },
            LedgerEntry {
                sequence: 2,
                logical_time: 2,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "credit".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
                from: String::new(),
                to: "RevenueAccount".to_string(),
                from_before: crate::types::MoneyAmount::from_minor_units(0),
                from_after: crate::types::MoneyAmount::from_minor_units(0),
                to_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("140").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&initial_state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("is unbalanced"));
    }
    #[test]
    fn runtime_invariant_rejects_broken_ledger_sequence() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
            },
            LedgerEntry {
                sequence: 3,
                logical_time: 3,
                transaction_sequence: 2,
                transaction: "Settlement".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("ledger sequence expected 2, found 3")
        );
    }
    #[test]
    fn runtime_invariant_rejects_transaction_name_change_within_sequence() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
            },
            LedgerEntry {
                sequence: 2,
                logical_time: 2,
                transaction_sequence: 1,
                transaction: "DifferentTransaction".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: "Merchant".to_string(),
                to: "Customer".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("350").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("changes transaction name"));
    }
    #[test]
    fn runtime_invariant_rejects_reversed_transaction_sequence() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("290").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("210").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("350").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
            },
            LedgerEntry {
                sequence: 2,
                logical_time: 2,
                transaction_sequence: 2,
                transaction: "Settlement".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: "Merchant".to_string(),
                to: "Customer".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
            },
            LedgerEntry {
                sequence: 3,
                logical_time: 3,
                transaction_sequence: 1,
                transaction: "Reversal".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("10").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("290").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("210").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid transaction sequence"));
    }
    #[test]
    fn runtime_invariant_rejects_invalid_transaction_sequence() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("350").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
            },
            LedgerEntry {
                sequence: 2,
                logical_time: 2,
                transaction_sequence: 3,
                transaction: "Settlement".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
                from: "Merchant".to_string(),
                to: "Customer".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("200").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("250").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("300").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid transaction sequence"));
    }
    #[test]
    fn runtime_invariant_rejects_invalid_logical_time() {
        let initial_state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
                    },
                ),
            ]),
        };
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![LedgerEntry {
            sequence: 1,
            logical_time: 2,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
        }];
        let result = validate_execution_invariants(&initial_state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("logical time"));
    }
    #[test]
    fn runtime_invariant_rejects_invalid_balance_transition() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("450").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
        }];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FP2010 INVARIANT_VIOLATION"));
    }
    #[test]
    fn runtime_invariant_rejects_broken_account_balance_chain() {
        let state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![
            LedgerEntry {
                sequence: 1,
                logical_time: 1,
                transaction_sequence: 1,
                transaction: "Sale".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            },
            LedgerEntry {
                sequence: 2,
                logical_time: 2,
                transaction_sequence: 2,
                transaction: "Settlement".to_string(),
                operation: "pay".to_string(),
                amount: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
                from: "Merchant".to_string(),
                to: "Customer".to_string(),
                from_before: crate::types::MoneyAmount::from_decimal_str("150").unwrap(),
                from_after: crate::types::MoneyAmount::from_decimal_str("50").unwrap(),
                to_before: crate::types::MoneyAmount::from_decimal_str("400").unwrap(),
                to_after: crate::types::MoneyAmount::from_decimal_str("500").unwrap(),
            },
        ];
        let result = validate_execution_invariants(&state, &state, &ledger);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("does not continue from its previous balance")
        );
    }
    #[test]
    fn runtime_invariant_rejects_negative_final_balance() {
        let initial_state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
                    },
                ),
            ]),
        };
        let final_state = ExecutionState {
            accounts: HashMap::from([
                (
                    "Customer".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("-10").unwrap(),
                    },
                ),
                (
                    "Merchant".to_string(),
                    AccountState {
                        account_type: AccountType::Asset,
                        currency: "USD".to_string(),
                        balance: crate::types::MoneyAmount::from_decimal_str("110").unwrap(),
                    },
                ),
            ]),
        };
        let ledger = vec![LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: crate::types::MoneyAmount::from_decimal_str("110").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
            from_after: crate::types::MoneyAmount::from_decimal_str("-10").unwrap(),
            to_before: crate::types::MoneyAmount::from_decimal_str("0").unwrap(),
            to_after: crate::types::MoneyAmount::from_decimal_str("110").unwrap(),
        }];
        let result = validate_execution_invariants(&initial_state, &final_state, &ledger);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("has a negative balance"));
    }
}
#[cfg(test)]
mod rollback_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction,
    };
    #[test]
    fn transaction_rolls_back_on_failure() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "AtomicSale".to_string(),
                    statements: vec![
                        Statement::Pay(Payment {
                            amount: Expression::Money(MoneyLiteral {
                                amount: "60".to_string(),
                                currency: "USD".to_string(),
                            }),
                            from: "Customer".to_string(),
                            to: "Merchant".to_string(),
                        }),
                        Statement::Pay(Payment {
                            amount: Expression::Money(MoneyLiteral {
                                amount: "60".to_string(),
                                currency: "USD".to_string(),
                            }),
                            from: "Customer".to_string(),
                            to: "Merchant".to_string(),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FP2006 INSUFFICIENT_FUNDS"));
    }
}
#[cfg(test)]
mod ledger_consistency_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction,
    };
    #[test]
    fn ledger_matches_final_state() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(50000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![Statement::Pay(Payment {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "100".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let result = execute(&program).expect("execution should succeed");
        assert_eq!(result.ledger.len(), 1);
        let entry = &result.ledger[0];
        assert_eq!(
            entry.from_before,
            crate::types::MoneyAmount::from_decimal_str("500").unwrap()
        );
        assert_eq!(
            entry.from_after,
            crate::types::MoneyAmount::from_decimal_str("400").unwrap()
        );
        assert_eq!(
            entry.to_before,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
        assert_eq!(
            entry.to_after,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.state.accounts["Customer"].balance, entry.from_after);
        assert_eq!(result.state.accounts["Merchant"].balance, entry.to_after);
    }
}
#[cfg(test)]
mod multi_operation_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction,
    };
    #[test]
    fn multiple_operations_in_one_transaction() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(50000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Bank".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "Settlement".to_string(),
                    statements: vec![
                        Statement::Pay(Payment {
                            amount: Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            }),
                            from: "Customer".to_string(),
                            to: "Merchant".to_string(),
                        }),
                        Statement::Pay(Payment {
                            amount: Expression::Money(MoneyLiteral {
                                amount: "60".to_string(),
                                currency: "USD".to_string(),
                            }),
                            from: "Merchant".to_string(),
                            to: "Bank".to_string(),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program).expect("execution should succeed");
        assert_eq!(result.ledger.len(), 2);
        assert_eq!(result.ledger[0].sequence, 1);
        assert_eq!(result.ledger[1].sequence, 2);
        assert_eq!(result.ledger[0].transaction_sequence, 1);
        assert_eq!(result.ledger[1].transaction_sequence, 1);
        assert_eq!(
            result.ledger[0].from_after,
            crate::types::MoneyAmount::from_decimal_str("400").unwrap()
        );
        assert_eq!(
            result.ledger[0].to_after,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(
            result.ledger[1].from_before,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(
            result.ledger[1].from_after,
            crate::types::MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(
            result.ledger[1].to_after,
            crate::types::MoneyAmount::from_decimal_str("60").unwrap()
        );
        assert_eq!(
            result.state.accounts["Customer"].balance,
            crate::types::MoneyAmount::from_decimal_str("400").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            crate::types::MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(
            result.state.accounts["Bank"].balance,
            crate::types::MoneyAmount::from_decimal_str("60").unwrap()
        );
    }
}
#[cfg(test)]
mod transaction_order_tests {
    use super::*;
    use crate::ast::Transfer;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction,
    };
    #[test]
    fn ledger_preserves_transaction_order() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(50000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Bank".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![Statement::Pay(Payment {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "100".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
                Declaration::Transaction(Transaction {
                    name: "Settlement".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "60".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Merchant".to_string(),
                        to: "Bank".to_string(),
                    })],
                }),
            ],
        };
        let result = execute(&program).expect("execution should succeed");
        assert_eq!(result.ledger.len(), 2);
        assert_eq!(result.ledger[0].sequence, 1);
        assert_eq!(result.ledger[1].sequence, 2);
        assert_eq!(result.ledger[0].transaction_sequence, 1);
        assert_eq!(result.ledger[1].transaction_sequence, 2);
        assert_eq!(result.ledger[0].transaction, "Sale");
        assert_eq!(result.ledger[0].operation, "pay");
        assert_eq!(result.ledger[0].from, "Customer");
        assert_eq!(result.ledger[0].to, "Merchant");
        assert_eq!(
            result.ledger[0].amount,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.ledger[1].transaction, "Settlement");
        assert_eq!(result.ledger[1].operation, "transfer");
        assert_eq!(result.ledger[1].from, "Merchant");
        assert_eq!(result.ledger[1].to, "Bank");
        assert_eq!(
            result.ledger[1].amount,
            crate::types::MoneyAmount::from_decimal_str("60").unwrap()
        );
        assert_eq!(
            result.state.accounts["Customer"].balance,
            crate::types::MoneyAmount::from_decimal_str("400").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            crate::types::MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(
            result.state.accounts["Bank"].balance,
            crate::types::MoneyAmount::from_decimal_str("60").unwrap()
        );
    }
}
#[cfg(test)]
mod ledger_persistence_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction, Transfer,
    };
    #[test]
    fn ledger_keeps_all_successful_transactions() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(100000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Bank".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![Statement::Pay(Payment {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "300".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
                Declaration::Transaction(Transaction {
                    name: "Settlement".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "200".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Merchant".to_string(),
                        to: "Bank".to_string(),
                    })],
                }),
                Declaration::Transaction(Transaction {
                    name: "FinalSettlement".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "50".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Merchant".to_string(),
                        to: "Customer".to_string(),
                    })],
                }),
            ],
        };
        let result = execute(&program).expect("execution should succeed");
        assert_eq!(result.ledger.len(), 3);
        assert_eq!(result.ledger[0].sequence, 1);
        assert_eq!(result.ledger[1].sequence, 2);
        assert_eq!(result.ledger[2].sequence, 3);
        assert_eq!(result.ledger[0].transaction, "Sale");
        assert_eq!(result.ledger[1].transaction, "Settlement");
        assert_eq!(result.ledger[2].transaction, "FinalSettlement");
        assert_eq!(
            result.ledger[0].amount,
            crate::types::MoneyAmount::from_decimal_str("300").unwrap()
        );
        assert_eq!(
            result.ledger[1].amount,
            crate::types::MoneyAmount::from_decimal_str("200").unwrap()
        );
        assert_eq!(
            result.ledger[2].amount,
            crate::types::MoneyAmount::from_decimal_str("50").unwrap()
        );
        assert_eq!(
            result.state.accounts["Customer"].balance,
            crate::types::MoneyAmount::from_decimal_str("750").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            crate::types::MoneyAmount::from_decimal_str("50").unwrap()
        );
        assert_eq!(
            result.state.accounts["Bank"].balance,
            crate::types::MoneyAmount::from_decimal_str("200").unwrap()
        );
    }
}
#[cfg(test)]
mod ledger_atomicity_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction,
    };
    #[test]
    fn failed_transaction_leaves_no_ledger_entries() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "AtomicSale".to_string(),
                    statements: vec![
                        Statement::Pay(Payment {
                            amount: Expression::Money(MoneyLiteral {
                                amount: "60".to_string(),
                                currency: "USD".to_string(),
                            }),
                            from: "Customer".to_string(),
                            to: "Merchant".to_string(),
                        }),
                        Statement::Pay(Payment {
                            amount: Expression::Money(MoneyLiteral {
                                amount: "60".to_string(),
                                currency: "USD".to_string(),
                            }),
                            from: "Customer".to_string(),
                            to: "Merchant".to_string(),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("FP2006 INSUFFICIENT_FUNDS"));
    }
}
#[cfg(test)]
mod zero_balance_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Payment, Program,
        Statement, Transaction,
    };
    #[test]
    fn allows_exact_balance_payment() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "FullPayment".to_string(),
                    statements: vec![Statement::Pay(Payment {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "100".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let result = execute(&program).expect("execution should succeed");
        assert_eq!(
            result.state.accounts["Customer"].balance,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.ledger.len(), 1);
        assert_eq!(
            result.ledger[0].from_after,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
    }
}
#[cfg(test)]
mod transfer_exact_balance_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn allows_exact_balance_transfer() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "FullTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "100".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let result = execute(&program).expect("execution should succeed");
        assert_eq!(
            result.state.accounts["Customer"].balance,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.ledger.len(), 1);
        assert_eq!(result.ledger[0].operation, "transfer");
        assert_eq!(
            result.ledger[0].from_after,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
    }
}
#[cfg(test)]
mod transfer_insufficient_funds_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_transfer_with_insufficient_funds() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(5000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "TooLargeTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "100".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2006 INSUFFICIENT_FUNDS: account 'Customer' has 50.00, but 100.00 is required."
        );
    }
}
#[cfg(test)]
mod transfer_currency_mismatch_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_transfer_between_different_currencies() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "EUR".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "CrossCurrencyTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "50".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2005 CURRENCY_MISMATCH: cannot move funds from USD to EUR."
        );
    }
}
#[cfg(test)]
mod transfer_unknown_account_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_transfer_to_unknown_account() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Transaction(Transaction {
                    name: "UnknownDestination".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "50".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Unknown".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2001 UNKNOWN_ACCOUNT: account 'Unknown' is not defined."
        );
    }
}
#[cfg(test)]
mod transfer_unknown_source_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_transfer_from_unknown_account() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "UnknownSource".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "50".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Unknown".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2001 UNKNOWN_ACCOUNT: account 'Unknown' is not defined."
        );
    }
}
#[cfg(test)]
mod transfer_self_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_transfer_to_same_account() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Transaction(Transaction {
                    name: "SelfTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "50".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Customer".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2004 SELF_TRANSFER: account 'Customer' cannot be both source and destination."
        );
    }
}
#[cfg(test)]
mod transfer_zero_amount_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_zero_transfer_amount() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "ZeroTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "0".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2003 INVALID_AMOUNT: transfer amount must be greater than zero."
        );
    }
}
#[cfg(test)]
mod transfer_negative_amount_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_negative_transfer_amount() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "NegativeTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "-50".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2003 INVALID_AMOUNT: transfer amount must be greater than zero."
        );
    }
}
#[cfg(test)]
mod transfer_expression_currency_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, Declaration, Expression, MoneyLiteral, Program, Statement,
        Transaction, Transfer,
    };
    #[test]
    fn rejects_transfer_with_wrong_expression_currency() {
        let program = Program {
            declarations: vec![
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "WrongCurrencyTransfer".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: Expression::Money(MoneyLiteral {
                            amount: "50".to_string(),
                            currency: "EUR".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        let error = execute(&program).expect_err("execution should fail");
        assert_eq!(
            error,
            "FP2005 CURRENCY_MISMATCH: expression uses EUR, but account uses USD."
        );
    }
}
#[cfg(test)]
mod double_entry_execution_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, CurrencyDeclaration, Declaration, Expression,
        MoneyLiteral, Statement, Transaction,
    };
    #[test]
    fn executes_balanced_double_entry() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "SalesRevenue".to_string(),
                    account_type: AccountType::Revenue,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(crate::ast::Debit {
                            account: "Cash".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "SalesRevenue".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program).unwrap();
        assert_eq!(
            result.state.accounts["Cash"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(
            result.state.accounts["SalesRevenue"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.ledger.len(), 2);
        assert_eq!(result.ledger[0].operation, "debit");
        assert_eq!(result.ledger[1].operation, "credit");
    }
}
#[cfg(test)]
mod double_entry_rollback_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, CurrencyDeclaration, Declaration, Expression,
        MoneyLiteral, Statement, Transaction,
    };
    #[test]
    fn rolls_back_double_entry_transaction_on_runtime_failure() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Equipment".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "SalesRevenue".to_string(),
                    account_type: AccountType::Revenue,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "RuntimeRollback".to_string(),
                    statements: vec![
                        Statement::Debit(crate::ast::Debit {
                            account: "Equipment".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "50".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "SalesRevenue".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "50".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Debit(crate::ast::Debit {
                            account: "SalesRevenue".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "200".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "Cash".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "200".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.contains("FP2006 INSUFFICIENT_FUNDS"));
        assert!(error.contains("SalesRevenue"));
        let mut state = ExecutionState::new();
        state.accounts.insert(
            "Cash".to_string(),
            AccountState {
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                balance: crate::types::MoneyAmount::from_minor_units(10000),
            },
        );
        state.accounts.insert(
            "Equipment".to_string(),
            AccountState {
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                balance: crate::types::MoneyAmount::from_minor_units(0),
            },
        );
        state.accounts.insert(
            "SalesRevenue".to_string(),
            AccountState {
                account_type: AccountType::Revenue,
                currency: "USD".to_string(),
                balance: crate::types::MoneyAmount::from_minor_units(0),
            },
        );
        assert_eq!(
            state.accounts["Cash"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(
            state.accounts["Equipment"].balance,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
        assert_eq!(
            state.accounts["SalesRevenue"].balance,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
    }
}
#[cfg(test)]
mod double_entry_account_type_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, CurrencyDeclaration, Declaration, Expression,
        MoneyLiteral, Statement, Transaction,
    };
    #[test]
    fn applies_debit_liability_and_equity_and_credit_asset() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Payable".to_string(),
                    account_type: AccountType::Liability,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "OwnerEquity".to_string(),
                    account_type: AccountType::Equity,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Transaction(Transaction {
                    name: "ReduceBalances".to_string(),
                    statements: vec![
                        Statement::Debit(crate::ast::Debit {
                            account: "Payable".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "50".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Debit(crate::ast::Debit {
                            account: "OwnerEquity".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "50".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "Cash".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program).unwrap();
        assert_eq!(
            result.state.accounts["Cash"].balance,
            crate::types::MoneyAmount::from_decimal_str("0").unwrap()
        );
        assert_eq!(
            result.state.accounts["Payable"].balance,
            crate::types::MoneyAmount::from_decimal_str("50").unwrap()
        );
        assert_eq!(
            result.state.accounts["OwnerEquity"].balance,
            crate::types::MoneyAmount::from_decimal_str("50").unwrap()
        );
        assert_eq!(result.ledger.len(), 3);
        assert_eq!(result.ledger[0].sequence, 1);
        assert_eq!(result.ledger[1].sequence, 2);
        assert_eq!(result.ledger[2].sequence, 3);
    }
}
#[cfg(test)]
mod decimal_money_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, CurrencyDeclaration, Declaration, Expression,
        MoneyLiteral, Statement, Transaction,
    };
    #[test]
    fn evaluates_decimal_expression_exactly() {
        let expression = Expression::Binary {
            left: Box::new(Expression::Money(MoneyLiteral {
                amount: "0.10".to_string(),
                currency: "USD".to_string(),
            })),
            operator: crate::ast::BinaryOperator::Add,
            right: Box::new(Expression::Money(MoneyLiteral {
                amount: "0.20".to_string(),
                currency: "USD".to_string(),
            })),
        };
        let result = evaluate_money_expression(&expression).unwrap();
        assert_eq!(result.minor_units(), 30);
    }
    #[test]
    fn money_amount_addition_is_exact() {
        let left = crate::types::MoneyAmount::from_minor_units(10);
        let right = crate::types::MoneyAmount::from_minor_units(20);
        let result = left
            .checked_add(right)
            .expect("test values must not overflow");
        assert_eq!(result.minor_units(), 30);
    }
    #[test]
    fn executes_decimal_money_amounts() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "SalesRevenue".to_string(),
                    account_type: AccountType::Revenue,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "DecimalSale".to_string(),
                    statements: vec![
                        Statement::Debit(crate::ast::Debit {
                            account: "Cash".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "0.10".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "SalesRevenue".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "0.10".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program).unwrap();
        assert_eq!(
            result.state.accounts["Cash"].balance,
            crate::types::MoneyAmount::from_decimal_str("0.10").unwrap()
        );
        assert_eq!(
            result.state.accounts["SalesRevenue"].balance,
            crate::types::MoneyAmount::from_decimal_str("0.10").unwrap()
        );
    }
}
#[cfg(test)]
mod double_entry_multi_currency_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, CurrencyDeclaration, Declaration, Expression,
        MoneyLiteral, Statement, Transaction,
    };
    #[test]
    fn executes_balanced_double_entry_in_multiple_currencies() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Currency(CurrencyDeclaration {
                    code: "EUR".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "CashUSD".to_string(),
                    account_type: AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "CashEUR".to_string(),
                    account_type: AccountType::Asset,
                    currency: "EUR".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "RevenueUSD".to_string(),
                    account_type: AccountType::Revenue,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "RevenueEUR".to_string(),
                    account_type: AccountType::Revenue,
                    currency: "EUR".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "MultiCurrency".to_string(),
                    statements: vec![
                        Statement::Debit(crate::ast::Debit {
                            account: "CashUSD".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "RevenueUSD".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            }),
                        }),
                        Statement::Debit(crate::ast::Debit {
                            account: "CashEUR".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "200".to_string(),
                                currency: "EUR".to_string(),
                            }),
                        }),
                        Statement::Credit(crate::ast::Credit {
                            account: "RevenueEUR".to_string(),
                            amount: Expression::Money(MoneyLiteral {
                                amount: "200".to_string(),
                                currency: "EUR".to_string(),
                            }),
                        }),
                    ],
                }),
            ],
        };
        let result = execute(&program).unwrap();
        assert_eq!(
            result.state.accounts["CashUSD"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(
            result.state.accounts["RevenueUSD"].balance,
            crate::types::MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(
            result.state.accounts["CashEUR"].balance,
            crate::types::MoneyAmount::from_decimal_str("200").unwrap()
        );
        assert_eq!(
            result.state.accounts["RevenueEUR"].balance,
            crate::types::MoneyAmount::from_decimal_str("200").unwrap()
        );
        assert_eq!(result.ledger.len(), 4);
    }
}
