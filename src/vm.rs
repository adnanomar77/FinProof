use crate::bytecode::{BytecodeInstruction, BytecodeProgram};
use crate::runtime::{
    AccountState, ExecutionResult, ExecutionState, LedgerEntry, validate_execution_invariants,
};
use crate::types::MoneyAmount;

#[derive(Debug, Clone, PartialEq)]
struct StackMoney {
    amount: MoneyAmount,
    currency: String,
}

pub fn execute_bytecode(program: &BytecodeProgram) -> Result<ExecutionResult, String> {
    let mut stack: Vec<StackMoney> = Vec::new();
    let mut state = ExecutionState::new();
    let mut ledger: Vec<LedgerEntry> = Vec::new();
    let mut initial_state: Option<ExecutionState> = None;
    let mut transaction_state: Option<ExecutionState> = None;
    let mut transaction_ledger: Option<Vec<LedgerEntry>> = None;
    let mut active_transaction: Option<(String, u64)> = None;
    let mut last_committed_transaction_sequence: u64 = 0;
    let mut trace: Vec<crate::verification::ExecutionTraceEntry> = Vec::new();
    let mut trace_step: u64 = 0;

    for instruction in &program.instructions {
        let pre_state = transaction_state.as_ref().unwrap_or(&state).clone();
        let pre_stack = stack.clone();

        match instruction {
            BytecodeInstruction::InitAccount {
                name,
                account_type,
                currency,
                initial_balance,
            } => {
                if active_transaction.is_some() {
                    return Err(
                        "FP3006 VM_STATE_ERROR: account initialization is not allowed inside a transaction."
                            .to_string(),
                    );
                }

                if state.accounts.contains_key(name) {
                    return Err(format!(
                        "FP2002 DUPLICATE_ACCOUNT: account '{}' already exists.",
                        name
                    ));
                }

                if initial_balance.minor_units() < 0 {
                    return Err(format!(
                        "FP2008 INVALID_INITIAL_BALANCE: account '{}' cannot have a negative initial balance.",
                        name
                    ));
                }

                state.accounts.insert(
                    name.clone(),
                    AccountState {
                        account_type: account_type.clone(),
                        currency: currency.clone(),
                        balance: *initial_balance,
                    },
                );
            }

            BytecodeInstruction::BeginTransaction { name, sequence } => {
                if active_transaction.is_some() {
                    return Err(
                        "FP3006 VM_STATE_ERROR: cannot begin a transaction while another transaction is active."
                            .to_string(),
                    );
                }

                if *sequence == 0 {
                    return Err(
                        "FP3007 VM_TRANSACTION_SEQUENCE: transaction sequence must start at 1."
                            .to_string(),
                    );
                }
                let expected_sequence = last_committed_transaction_sequence
                    .checked_add(1)
                    .ok_or_else(|| {
                        "FP3007 VM_TRANSACTION_SEQUENCE: transaction sequence overflow.".to_string()
                    })?;

                if *sequence != expected_sequence {
                    return Err(format!(
                        "FP3007 VM_TRANSACTION_SEQUENCE: expected transaction sequence {}, found {}.",
                        expected_sequence, sequence
                    ));
                }

                if initial_state.is_none() {
                    initial_state = Some(state.clone());
                }

                transaction_state = Some(state.clone());
                transaction_ledger = Some(ledger.clone());

                active_transaction = Some((name.clone(), *sequence));
            }

            BytecodeInstruction::EndTransaction => {
                let (transaction_name, transaction_sequence) =
                    active_transaction.clone().ok_or_else(|| {
                        "FP3006 VM_STATE_ERROR: cannot end a transaction when none is active."
                            .to_string()
                    })?;

                if !stack.is_empty() {
                    return Err(
                        "FP3004 VM_STACK_ERROR: transaction ended with unconsumed monetary values."
                            .to_string(),
                    );
                }

                let transaction_state = transaction_state.take().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction state is missing.".to_string()
                })?;

                let transaction_ledger = transaction_ledger.take().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction ledger is missing.".to_string()
                })?;

                validate_execution_invariants(
                    initial_state.as_ref().ok_or_else(|| {
                        "FP3006 VM_STATE_ERROR: initial execution state is missing.".to_string()
                    })?,
                    &transaction_state,
                    &transaction_ledger,
                )?;

                state = transaction_state;
                ledger = transaction_ledger;
                last_committed_transaction_sequence = transaction_sequence;
                active_transaction = None;

                let _ = transaction_name;
            }

            BytecodeInstruction::PushMoney { amount, currency } => {
                stack.push(StackMoney {
                    amount: *amount,
                    currency: currency.clone(),
                });
            }

            BytecodeInstruction::Add => {
                let right = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Add requires two monetary values.".to_string()
                })?;

                let left = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Add requires two monetary values.".to_string()
                })?;

                if left.currency != right.currency {
                    return Err(format!(
                        "FP3005 VM_CURRENCY_MISMATCH: cannot add {} and {}.",
                        left.currency, right.currency
                    ));
                }

                let amount = left.amount.checked_add(right.amount).map_err(|_| {
                    "FP3009 VM_ARITHMETIC_OVERFLOW: monetary addition exceeds the supported range."
                        .to_string()
                })?;

                stack.push(StackMoney {
                    amount,
                    currency: left.currency,
                });
            }

            BytecodeInstruction::Subtract => {
                let right = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Subtract requires two monetary values.".to_string()
                })?;

                let left = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Subtract requires two monetary values.".to_string()
                })?;

                if left.currency != right.currency {
                    return Err(format!(
                        "FP3005 VM_CURRENCY_MISMATCH: cannot subtract {} and {}.",
                        left.currency, right.currency
                    ));
                }

                let amount = left.amount.checked_sub(right.amount).map_err(|_| {
                    "FP3009 VM_ARITHMETIC_OVERFLOW: monetary subtraction exceeds the supported range."
                        .to_string()
                })?;

                stack.push(StackMoney {
                    amount,
                    currency: left.currency,
                });
            }

            BytecodeInstruction::Pay { from, to } => {
                let value = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Pay requires one monetary value.".to_string()
                })?;

                if value.amount.minor_units() <= 0 {
                    return Err(
                        "FP2003 INVALID_AMOUNT: pay amount must be greater than zero.".to_string(),
                    );
                }

                if from == to {
                    return Err(format!(
                        "FP2004 SELF_TRANSFER: account '{}' cannot be both source and destination.",
                        from
                    ));
                }

                let active_transaction = active_transaction.as_ref().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: Pay is only allowed inside a transaction.".to_string()
                })?;

                let current_state = transaction_state.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction state is missing.".to_string()
                })?;

                let current_ledger = transaction_ledger.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction ledger is missing.".to_string()
                })?;

                let from_account = current_state.accounts.get(from).ok_or_else(|| {
                    format!("FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.", from)
                })?;

                let from_currency = from_account.currency.clone();
                let from_balance = from_account.balance;

                let to_account = current_state.accounts.get(to).ok_or_else(|| {
                    format!("FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.", to)
                })?;

                let to_currency = to_account.currency.clone();
                let to_balance = to_account.balance;

                if from_currency != to_currency {
                    return Err(format!(
                        "FP2005 CURRENCY_MISMATCH: cannot move funds from {} to {}.",
                        from_currency, to_currency
                    ));
                }

                if value.currency != from_currency {
                    return Err(format!(
                        "FP2005 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
                        value.currency, from_currency
                    ));
                }

                if from_balance < value.amount {
                    return Err(format!(
                        "FP2006 INSUFFICIENT_FUNDS: account '{}' has {}, but {} is required.",
                        from, from_balance, value.amount
                    ));
                }

                let from_after = from_balance.checked_sub(value.amount).map_err(|_| {
                    format!(
                        "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                        from
                    )
                })?;

                let to_after = to_balance.checked_add(value.amount).map_err(|_| {
                    format!(
                        "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                        to
                    )
                })?;

                current_state.accounts.get_mut(from).unwrap().balance = from_after;
                current_state.accounts.get_mut(to).unwrap().balance = to_after;

                current_ledger.push(LedgerEntry {
                    sequence: (current_ledger.len() + 1) as u64,
                    logical_time: (current_ledger.len() + 1) as u64,
                    transaction_sequence: active_transaction.1,
                    transaction: active_transaction.0.clone(),
                    operation: "pay".to_string(),
                    amount: value.amount,
                    currency: from_currency,
                    from: from.clone(),
                    to: to.clone(),
                    from_before: from_balance,
                    from_after,
                    to_before: to_balance,
                    to_after,
                });
            }

            BytecodeInstruction::Transfer { from, to } => {
                let value = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Transfer requires one monetary value.".to_string()
                })?;

                if value.amount.minor_units() <= 0 {
                    return Err(
                        "FP2003 INVALID_AMOUNT: transfer amount must be greater than zero."
                            .to_string(),
                    );
                }

                if from == to {
                    return Err(format!(
                        "FP2004 SELF_TRANSFER: account '{}' cannot be both source and destination.",
                        from
                    ));
                }

                let active_transaction = active_transaction.as_ref().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: Transfer is only allowed inside a transaction."
                        .to_string()
                })?;

                let current_state = transaction_state.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction state is missing.".to_string()
                })?;

                let current_ledger = transaction_ledger.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction ledger is missing.".to_string()
                })?;

                let from_account = current_state.accounts.get(from).ok_or_else(|| {
                    format!("FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.", from)
                })?;

                let from_currency = from_account.currency.clone();
                let from_balance = from_account.balance;

                let to_account = current_state.accounts.get(to).ok_or_else(|| {
                    format!("FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.", to)
                })?;

                let to_currency = to_account.currency.clone();
                let to_balance = to_account.balance;

                if from_currency != to_currency {
                    return Err(format!(
                        "FP2005 CURRENCY_MISMATCH: cannot move funds from {} to {}.",
                        from_currency, to_currency
                    ));
                }

                if value.currency != from_currency {
                    return Err(format!(
                        "FP2005 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
                        value.currency, from_currency
                    ));
                }

                if from_balance < value.amount {
                    return Err(format!(
                        "FP2006 INSUFFICIENT_FUNDS: account '{}' has {}, but {} is required.",
                        from, from_balance, value.amount
                    ));
                }

                let from_after = from_balance.checked_sub(value.amount).map_err(|_| {
                    format!(
                        "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                        from
                    )
                })?;

                let to_after = to_balance.checked_add(value.amount).map_err(|_| {
                    format!(
                        "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                        to
                    )
                })?;

                current_state.accounts.get_mut(from).unwrap().balance = from_after;
                current_state.accounts.get_mut(to).unwrap().balance = to_after;

                current_ledger.push(LedgerEntry {
                    sequence: (current_ledger.len() + 1) as u64,
                    logical_time: (current_ledger.len() + 1) as u64,
                    transaction_sequence: active_transaction.1,
                    transaction: active_transaction.0.clone(),
                    operation: "transfer".to_string(),
                    amount: value.amount,
                    currency: from_currency,
                    from: from.clone(),
                    to: to.clone(),
                    from_before: from_balance,
                    from_after,
                    to_before: to_balance,
                    to_after,
                });
            }

            BytecodeInstruction::Debit { account } => {
                let value = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Debit requires one monetary value.".to_string()
                })?;

                if value.amount.minor_units() <= 0 {
                    return Err(
                        "FP2003 INVALID_AMOUNT: double-entry amount must be greater than zero."
                            .to_string(),
                    );
                }

                let active_transaction = active_transaction.as_ref().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: Debit is only allowed inside a transaction.".to_string()
                })?;

                let current_state = transaction_state.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction state is missing.".to_string()
                })?;

                let current_ledger = transaction_ledger.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction ledger is missing.".to_string()
                })?;

                let account_state = current_state.accounts.get(account).ok_or_else(|| {
                    format!(
                        "FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.",
                        account
                    )
                })?;

                let account_currency = account_state.currency.clone();
                let account_type = account_state.account_type.clone();
                let before = account_state.balance;

                if value.currency != account_currency {
                    return Err(format!(
                        "FP2005 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
                        value.currency, account_currency
                    ));
                }

                let increases = matches!(
                    account_type,
                    crate::ast::AccountType::Asset | crate::ast::AccountType::Expense
                );

                let after = if increases {
                    before.checked_add(value.amount).map_err(|_| {
                        format!(
                            "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                            account
                        )
                    })?
                } else {
                    if before.minor_units() < value.amount.minor_units() {
                        return Err(format!(
                            "FP2006 INSUFFICIENT_FUNDS: account '{}' has {}, but {} is required.",
                            account, before, value.amount
                        ));
                    }

                    before.checked_sub(value.amount).map_err(|_| {
                        format!(
                            "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                            account
                        )
                    })?
                };

                let zero = MoneyAmount::from_minor_units(0);

                current_state.accounts.get_mut(account).unwrap().balance = after;

                current_ledger.push(LedgerEntry {
                    sequence: (current_ledger.len() + 1) as u64,
                    logical_time: (current_ledger.len() + 1) as u64,
                    transaction_sequence: active_transaction.1,
                    transaction: active_transaction.0.clone(),
                    operation: "debit".to_string(),
                    amount: value.amount,
                    currency: account_currency,
                    from: account.clone(),
                    to: String::new(),
                    from_before: before,
                    from_after: after,
                    to_before: zero,
                    to_after: zero,
                });
            }

            BytecodeInstruction::Credit { account } => {
                let value = stack.pop().ok_or_else(|| {
                    "FP3003 VM_STACK_UNDERFLOW: Credit requires one monetary value.".to_string()
                })?;

                if value.amount.minor_units() <= 0 {
                    return Err(
                        "FP2003 INVALID_AMOUNT: double-entry amount must be greater than zero."
                            .to_string(),
                    );
                }

                let active_transaction = active_transaction.as_ref().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: Credit is only allowed inside a transaction."
                        .to_string()
                })?;

                let current_state = transaction_state.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction state is missing.".to_string()
                })?;

                let current_ledger = transaction_ledger.as_mut().ok_or_else(|| {
                    "FP3006 VM_STATE_ERROR: transaction ledger is missing.".to_string()
                })?;

                let account_state = current_state.accounts.get(account).ok_or_else(|| {
                    format!(
                        "FP2001 UNKNOWN_ACCOUNT: account '{}' is not defined.",
                        account
                    )
                })?;

                let account_currency = account_state.currency.clone();
                let account_type = account_state.account_type.clone();
                let before = account_state.balance;

                if value.currency != account_currency {
                    return Err(format!(
                        "FP2005 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
                        value.currency, account_currency
                    ));
                }

                let increases = matches!(
                    account_type,
                    crate::ast::AccountType::Liability
                        | crate::ast::AccountType::Equity
                        | crate::ast::AccountType::Revenue
                );

                let after = if increases {
                    before.checked_add(value.amount).map_err(|_| {
                        format!(
                            "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                            account
                        )
                    })?
                } else {
                    if before.minor_units() < value.amount.minor_units() {
                        return Err(format!(
                            "FP2006 INSUFFICIENT_FUNDS: account '{}' has {}, but {} is required.",
                            account, before, value.amount
                        ));
                    }

                    before.checked_sub(value.amount).map_err(|_| {
                        format!(
                            "FP3009 VM_ARITHMETIC_OVERFLOW: balance update for account '{}' exceeds the supported monetary range.",
                            account
                        )
                    })?
                };

                let zero = MoneyAmount::from_minor_units(0);

                current_state.accounts.get_mut(account).unwrap().balance = after;

                current_ledger.push(LedgerEntry {
                    sequence: (current_ledger.len() + 1) as u64,
                    logical_time: (current_ledger.len() + 1) as u64,
                    transaction_sequence: active_transaction.1,
                    transaction: active_transaction.0.clone(),
                    operation: "credit".to_string(),
                    amount: value.amount,
                    currency: account_currency,
                    from: String::new(),
                    to: account.clone(),
                    from_before: zero,
                    from_after: zero,
                    to_before: before,
                    to_after: after,
                });
            }
        }
        trace_step = trace_step
            .checked_add(1)
            .ok_or_else(|| "FP3009 VM_ARITHMETIC_OVERFLOW: trace step overflow.".to_string())?;

        let post_state = transaction_state.as_ref().unwrap_or(&state).clone();

        let pre_stack_entries = pre_stack
            .iter()
            .map(|entry| (entry.amount, entry.currency.clone()))
            .collect::<Vec<_>>();

        let post_stack_entries = stack
            .iter()
            .map(|entry| (entry.amount, entry.currency.clone()))
            .collect::<Vec<_>>();

        trace.push(crate::verification::ExecutionTraceEntry {
            step: trace_step,
            instruction: instruction.canonical_representation(),
            pre_state_hash: crate::verification::state_hash(&pre_state),
            post_state_hash: crate::verification::state_hash(&post_state),
            pre_stack_hash: crate::verification::stack_hash(&pre_stack_entries),
            post_stack_hash: crate::verification::stack_hash(&post_stack_entries),
            pre_state,
            post_state,
            pre_stack: pre_stack_entries,
            post_stack: post_stack_entries,
        });
    }

    if active_transaction.is_some() {
        return Err(
            "FP3006 VM_STATE_ERROR: bytecode ended while a transaction was still active."
                .to_string(),
        );
    }

    if !stack.is_empty() {
        return Err(
            "FP3004 VM_STACK_ERROR: bytecode ended with unconsumed monetary values.".to_string(),
        );
    }

    if let Some(initial_state) = &initial_state {
        validate_execution_invariants(initial_state, &state, &ledger)?;
    }

    Ok(ExecutionResult {
        state,
        ledger,
        trace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::AccountType;

    #[test]
    fn vm_rejects_unconsumed_push_money() {
        let program = BytecodeProgram::new(vec![BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_decimal_str("100").unwrap(),
            currency: "USD".to_string(),
        }]);

        let error = execute_bytecode(&program).unwrap_err();
        assert!(error.contains("FP3004 VM_STACK_ERROR"));
    }

    #[test]
    fn vm_rejects_unconsumed_add_result() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("25").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Add,
        ]);

        let error = execute_bytecode(&program).unwrap_err();
        assert!(error.contains("FP3004 VM_STACK_ERROR"));
    }

    #[test]
    fn vm_rejects_unconsumed_subtract_result() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("25").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Subtract,
        ]);

        let error = execute_bytecode(&program).unwrap_err();
        assert!(error.contains("FP3004 VM_STACK_ERROR"));
    }

    #[test]
    fn vm_rejects_mixed_currency_addition() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("25").unwrap(),
                currency: "EUR".to_string(),
            },
            BytecodeInstruction::Add,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP3005 VM_CURRENCY_MISMATCH"));
    }

    #[test]
    fn vm_initializes_account() {
        let program = BytecodeProgram::new(vec![BytecodeInstruction::InitAccount {
            name: "Customer".to_string(),
            account_type: AccountType::Asset,
            currency: "USD".to_string(),
            initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
        }]);

        assert!(execute_bytecode(&program).is_ok());
    }
    #[test]
    fn vm_rejects_first_transaction_sequence_not_one() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 2,
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert_eq!(
            error,
            "FP3007 VM_TRANSACTION_SEQUENCE: expected transaction sequence 1, found 2."
        );
    }

    #[test]
    fn vm_rejects_transaction_sequence_gap() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::EndTransaction,
            BytecodeInstruction::BeginTransaction {
                name: "Purchase".to_string(),
                sequence: 3,
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert_eq!(
            error,
            "FP3007 VM_TRANSACTION_SEQUENCE: expected transaction sequence 2, found 3."
        );
    }

    #[test]
    fn vm_accepts_contiguous_transaction_sequences() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::EndTransaction,
            BytecodeInstruction::BeginTransaction {
                name: "Purchase".to_string(),
                sequence: 2,
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program);

        assert!(result.is_ok());
    }

    #[test]
    fn vm_executes_transaction_boundaries() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::EndTransaction,
        ]);

        assert!(execute_bytecode(&program).is_ok());
    }

    #[test]
    fn vm_rejects_nested_transaction() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::BeginTransaction {
                name: "Nested".to_string(),
                sequence: 2,
            },
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP3006 VM_STATE_ERROR"));
    }

    #[test]
    fn vm_executes_pay() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Customer"].balance,
            MoneyAmount::from_decimal_str("0").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.ledger.len(), 1);
        assert_eq!(result.ledger[0].operation, "pay");
        assert_eq!(
            result.ledger[0].amount,
            MoneyAmount::from_decimal_str("100").unwrap()
        );
        assert_eq!(result.ledger[0].currency, "USD");
    }

    #[test]
    fn vm_executes_debit() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "RevenueAccount".to_string(),
                account_type: AccountType::Revenue,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "DebitCash".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Debit {
                account: "Cash".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Credit {
                account: "RevenueAccount".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Cash"].balance,
            MoneyAmount::from_decimal_str("140").unwrap()
        );
        assert_eq!(result.ledger.len(), 2);
        assert_eq!(result.ledger[0].operation, "debit");
        assert_eq!(
            result.ledger[0].amount,
            MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(result.ledger[0].currency, "USD");
        assert_eq!(result.ledger[1].operation, "credit");
        assert_eq!(
            result.ledger[1].amount,
            MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(result.ledger[1].currency, "USD");
    }

    #[test]
    fn vm_executes_transfer() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Customer"].balance,
            MoneyAmount::from_decimal_str("60").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(result.ledger.len(), 1);
        assert_eq!(result.ledger[0].operation, "transfer");
        assert_eq!(
            result.ledger[0].amount,
            MoneyAmount::from_decimal_str("40").unwrap()
        );
        assert_eq!(result.ledger[0].currency, "USD");
    }
    #[test]
    fn vm_rejects_transfer_with_insufficient_funds() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("101").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2006 INSUFFICIENT_FUNDS"));
    }
    #[test]
    fn vm_rejects_zero_transfer_amount() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("0").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2003 INVALID_AMOUNT"));
    }
    #[test]
    fn vm_rejects_negative_transfer_amount() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("-1").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2003 INVALID_AMOUNT"));
    }
    #[test]
    fn vm_rejects_transfer_between_different_currencies() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "EUR".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2005 CURRENCY_MISMATCH"));
    }
    #[test]
    fn vm_rejects_transfer_to_unknown_account() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Unknown".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2001 UNKNOWN_ACCOUNT"));
    }
    #[test]
    fn vm_rejects_transfer_from_unknown_account() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Unknown".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2001 UNKNOWN_ACCOUNT"));
    }
    #[test]
    fn vm_rejects_transfer_to_same_account() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "TransferFunds".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Transfer {
                from: "Customer".to_string(),
                to: "Customer".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2004 SELF_TRANSFER"));
    }
    #[test]
    fn vm_rejects_zero_pay_amount() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("0").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2003 INVALID_AMOUNT"));
    }
    #[test]
    fn vm_rejects_negative_pay_amount() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("-1").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2003 INVALID_AMOUNT"));
    }
    #[test]
    fn vm_rejects_pay_with_insufficient_funds() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("101").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2006 INSUFFICIENT_FUNDS"));
    }
    #[test]
    fn vm_rejects_pay_between_different_currencies() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "EUR".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2005 CURRENCY_MISMATCH"));
    }
    #[test]
    fn vm_rejects_pay_to_unknown_account() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Unknown".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2001 UNKNOWN_ACCOUNT"));
    }
    #[test]
    fn vm_rejects_pay_from_unknown_account() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Unknown".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2001 UNKNOWN_ACCOUNT"));
    }
    #[test]
    fn vm_rejects_pay_to_same_account() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "SelfPay".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Customer".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2004 SELF_TRANSFER"));
    }
    #[test]
    fn vm_executes_decimal_pay_exactly() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100.25").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("10.10").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "DecimalPay".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("25.15").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Customer"].balance,
            MoneyAmount::from_decimal_str("75.10").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            MoneyAmount::from_decimal_str("35.25").unwrap()
        );
    }
    #[test]
    fn vm_executes_compound_pay_expression() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "CompoundPay".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("10").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("20").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Add,
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Customer"].balance,
            MoneyAmount::from_decimal_str("70").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            MoneyAmount::from_decimal_str("30").unwrap()
        );
    }
    #[test]
    fn vm_executes_compound_pay_subtraction() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "CompoundSubPay".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("50").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("20").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Subtract,
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Customer"].balance,
            MoneyAmount::from_decimal_str("70").unwrap()
        );
        assert_eq!(
            result.state.accounts["Merchant"].balance,
            MoneyAmount::from_decimal_str("30").unwrap()
        );
    }
    #[test]
    fn vm_rejects_compound_pay_mixed_currency() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "MixedPay".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("10").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("20").unwrap(),
                currency: "EUR".to_string(),
            },
            BytecodeInstruction::Add,
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP3005 VM_CURRENCY_MISMATCH"));
    }
    #[test]
    fn vm_executes_balanced_double_entry() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "SalesRevenue".to_string(),
                account_type: AccountType::Revenue,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Debit {
                account: "Cash".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Credit {
                account: "SalesRevenue".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Cash"].balance,
            MoneyAmount::from_decimal_str("140").unwrap()
        );
        assert_eq!(
            result.state.accounts["SalesRevenue"].balance,
            MoneyAmount::from_decimal_str("40").unwrap()
        );
    }
    #[test]
    fn vm_rejects_unbalanced_double_entry() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "SalesRevenue".to_string(),
                account_type: AccountType::Revenue,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "UnbalancedSale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Debit {
                account: "Cash".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("30").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Credit {
                account: "SalesRevenue".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2010 INVARIANT_VIOLATION"));
    }
    #[test]
    fn vm_executes_liability_and_asset_double_entry() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "LoanPayable".to_string(),
                account_type: AccountType::Liability,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("50").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "LoanReceipt".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Debit {
                account: "Cash".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Credit {
                account: "LoanPayable".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["Cash"].balance,
            MoneyAmount::from_decimal_str("90").unwrap()
        );
        assert_eq!(
            result.state.accounts["LoanPayable"].balance,
            MoneyAmount::from_decimal_str("140").unwrap()
        );
    }
    #[test]
    fn vm_executes_debit_liability() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "LoanPayable".to_string(),
                account_type: AccountType::Liability,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "LoanPayment".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Debit {
                account: "LoanPayable".to_string(),
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Credit {
                account: "Cash".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(
            result.state.accounts["LoanPayable"].balance,
            MoneyAmount::from_decimal_str("60").unwrap()
        );
        assert_eq!(
            result.state.accounts["Cash"].balance,
            MoneyAmount::from_decimal_str("60").unwrap()
        );
    }
    #[test]
    fn vm_rejects_credit_asset() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("100").unwrap(),
            },
            BytecodeInstruction::InitAccount {
                name: "Revenue".to_string(),
                account_type: AccountType::Revenue,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_decimal_str("0").unwrap(),
            },
            BytecodeInstruction::BeginTransaction {
                name: "InvalidCredit".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("40").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Credit {
                account: "Cash".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP2010 INVARIANT_VIOLATION"));
    }
    #[test]
    fn vm_rejects_unclosed_transaction() {
        let program = BytecodeProgram::new(vec![BytecodeInstruction::BeginTransaction {
            name: "Sale".to_string(),
            sequence: 1,
        }]);

        let error = execute_bytecode(&program).unwrap_err();

        assert!(error.contains("FP3006 VM_STATE_ERROR"));
    }

    #[test]
    fn vm_generates_execution_trace_for_each_opcode() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(10000),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(0),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_minor_units(10000),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        assert_eq!(result.trace.len(), program.instructions.len());

        for (index, entry) in result.trace.iter().enumerate() {
            assert_eq!(entry.step, (index + 1) as u64);
            assert_eq!(
                entry.instruction,
                program.instructions[index].canonical_representation()
            );
        }
    }
    #[test]
    fn vm_trace_hashes_match_actual_pre_and_post_values() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(10000),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(0),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_minor_units(10000),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result = execute_bytecode(&program).unwrap();

        let initial_state = crate::runtime::ExecutionState::new();
        let initial_state_hash = crate::verification::state_hash(&initial_state);
        let empty_stack_hash = crate::verification::stack_hash(&[]);

        assert_eq!(result.trace[0].pre_state_hash, initial_state_hash);
        assert_eq!(result.trace[0].pre_stack_hash, empty_stack_hash);

        assert_ne!(
            result.trace[0].pre_state_hash,
            result.trace[0].post_state_hash
        );

        assert_ne!(
            result.trace[3].pre_stack_hash,
            result.trace[3].post_stack_hash
        );

        assert_eq!(
            result.trace[3].post_stack_hash,
            result.trace[4].pre_stack_hash
        );
    }
    #[test]
    fn vm_trace_root_is_deterministic_and_execution_sensitive() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::InitAccount {
                name: "Customer".to_string(),
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(10000),
            },
            BytecodeInstruction::InitAccount {
                name: "Merchant".to_string(),
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(0),
            },
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_minor_units(10000),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let result_a = execute_bytecode(&program).unwrap();
        let result_b = execute_bytecode(&program).unwrap();

        let root_a = crate::verification::trace_root(&result_a.trace);
        let root_b = crate::verification::trace_root(&result_b.trace);

        assert_eq!(root_a, root_b);

        let mut changed_instructions = program.instructions.clone();
        changed_instructions[3] = BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_minor_units(5000),
            currency: "USD".to_string(),
        };

        let changed_program = BytecodeProgram::new(changed_instructions);
        let changed_result = execute_bytecode(&changed_program).unwrap();

        let changed_root = crate::verification::trace_root(&changed_result.trace);

        assert_ne!(root_a, changed_root);
    }
}
