use crate::bytecode::BytecodeProgram;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionTraceEntry {
    pub step: u64,
    pub instruction: String,
    pub pre_state_hash: [u8; 32],
    pub post_state_hash: [u8; 32],
    pub pre_stack_hash: [u8; 32],
    pub post_stack_hash: [u8; 32],
    pub pre_state: crate::runtime::ExecutionState,
    pub post_state: crate::runtime::ExecutionState,
    pub pre_stack: Vec<(crate::types::MoneyAmount, String)>,
    pub post_stack: Vec<(crate::types::MoneyAmount, String)>,
}

impl Default for ExecutionTraceEntry {
    fn default() -> Self {
        Self {
            step: 0,
            instruction: String::new(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [0u8; 32],
            pre_stack_hash: [0u8; 32],
            post_stack_hash: [0u8; 32],
            pre_state: crate::runtime::ExecutionState {
                accounts: std::collections::HashMap::new(),
            },
            post_state: crate::runtime::ExecutionState {
                accounts: std::collections::HashMap::new(),
            },
            pre_stack: Vec::new(),
            post_stack: Vec::new(),
        }
    }
}

impl ExecutionTraceEntry {
    #[allow(dead_code)]
    pub fn canonical_representation(&self) -> String {
        format!(
            "step={};instruction={};pre_state_hash={};post_state_hash={};pre_stack_hash={};post_stack_hash={}",
            self.step,
            self.instruction,
            hex_encode(&self.pre_state_hash),
            hex_encode(&self.post_state_hash),
            hex_encode(&self.pre_stack_hash),
            hex_encode(&self.post_stack_hash),
        )
    }
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}
#[allow(dead_code)]
pub fn trace_root(trace: &[ExecutionTraceEntry]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    for entry in trace {
        let entry_hash = {
            let canonical = entry.canonical_representation();

            let mut entry_hasher = Sha256::new();
            entry_hasher.update(canonical.as_bytes());
            entry_hasher.finalize()
        };

        hasher.update(entry_hash);
    }

    hasher.finalize().into()
}

#[allow(dead_code)]
pub fn trace_root_hex(trace: &[ExecutionTraceEntry]) -> String {
    trace_root(trace)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
pub fn stack_hash(entries: &[(crate::types::MoneyAmount, String)]) -> [u8; 32] {
    let canonical = entries
        .iter()
        .enumerate()
        .map(|(index, (amount, currency))| {
            format!(
                "Stack|index={}|amount_minor_units={}|currency={}",
                index,
                amount.minor_units(),
                currency
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());

    hasher.finalize().into()
}

#[allow(dead_code)]
pub fn transaction_hash(entry: &crate::runtime::LedgerEntry) -> [u8; 32] {
    let canonical = entry.canonical_representation();

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());

    hasher.finalize().into()
}

#[allow(dead_code)]
pub fn transaction_hash_hex(entry: &crate::runtime::LedgerEntry) -> String {
    transaction_hash(entry)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
pub fn state_hash(state: &crate::runtime::ExecutionState) -> [u8; 32] {
    let mut accounts = state.accounts.iter().collect::<Vec<_>>();

    accounts.sort_by_key(|(left_name, _)| *left_name);

    let canonical = accounts
        .iter()
        .map(|(name, account)| {
            let account_type = match account.account_type {
                crate::ast::AccountType::Asset => "Asset",
                crate::ast::AccountType::Liability => "Liability",
                crate::ast::AccountType::Equity => "Equity",
                crate::ast::AccountType::Revenue => "Revenue",
                crate::ast::AccountType::Expense => "Expense",
            };

            format!(
                "Account|name={}|account_type={}|currency={}|balance_minor_units={}",
                name,
                account_type,
                account.currency,
                account.balance.minor_units()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());

    hasher.finalize().into()
}

#[allow(dead_code)]
pub fn state_hash_hex(state: &crate::runtime::ExecutionState) -> String {
    state_hash(state)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
#[allow(dead_code)]
pub fn program_hash(program: &crate::ast::Program) -> [u8; 32] {
    let canonical = program.canonical_representation();

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());

    hasher.finalize().into()
}

#[allow(dead_code)]
pub fn program_hash_hex(program: &crate::ast::Program) -> String {
    program_hash(program)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
#[allow(dead_code)]
pub fn bytecode_hash_hex(program: &BytecodeProgram) -> String {
    bytecode_hash(program)
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}
pub fn bytecode_hash(program: &BytecodeProgram) -> [u8; 32] {
    let canonical = program.canonical_representation();

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());

    hasher.finalize().into()
}

pub fn verify_trace_continuity(trace: &[ExecutionTraceEntry]) -> Result<(), String> {
    for (index, entry) in trace.iter().enumerate() {
        let expected_step = u64::try_from(index)
            .map_err(|_| "FP4006 VERIFICATION_FAILED: trace index overflow.".to_string())?
            .checked_add(1)
            .ok_or_else(|| "FP4006 VERIFICATION_FAILED: trace step overflow.".to_string())?;

        if entry.step != expected_step {
            return Err(format!(
                "FP4006 VERIFICATION_FAILED: trace step discontinuity at index {}: expected {}, found {}.",
                index, expected_step, entry.step
            ));
        }

        if let Some(next) = trace.get(index + 1) {
            if entry.post_state_hash != next.pre_state_hash {
                return Err(format!(
                    "FP4007 VERIFICATION_FAILED: state transition discontinuity between trace steps {} and {}.",
                    entry.step, next.step
                ));
            }

            if entry.post_stack_hash != next.pre_stack_hash {
                return Err(format!(
                    "FP4008 VERIFICATION_FAILED: stack transition discontinuity between trace steps {} and {}.",
                    entry.step, next.step
                ));
            }
        }
    }

    Ok(())
}
pub fn verify_trace_matches_bytecode(
    trace: &[ExecutionTraceEntry],
    bytecode: &BytecodeProgram,
) -> Result<(), String> {
    if trace.len() != bytecode.instructions.len() {
        return Err(format!(
            "FP4009 VERIFICATION_FAILED: trace length mismatch: expected {}, found {}.",
            bytecode.instructions.len(),
            trace.len()
        ));
    }

    for (index, (entry, instruction)) in trace.iter().zip(bytecode.instructions.iter()).enumerate()
    {
        let expected_instruction = instruction.canonical_representation();

        if entry.instruction != expected_instruction {
            return Err(format!(
                "FP4010 VERIFICATION_FAILED: trace instruction mismatch at index {}: expected '{}', found '{}'.",
                index, expected_instruction, entry.instruction
            ));
        }
    }

    Ok(())
}
pub fn verify_execution_witness(
    bytecode: &BytecodeProgram,
    trace: &[ExecutionTraceEntry],
) -> Result<(), String> {
    if trace.len() != bytecode.instructions.len() {
        return Err(format!(
            "FP4015 VERIFICATION_FAILED: witness length mismatch: expected {}, found {}.",
            bytecode.instructions.len(),
            trace.len()
        ));
    }

    verify_trace_matches_bytecode(trace, bytecode)?;
    verify_trace_continuity(trace)?;

    let mut active_transaction: Option<(String, u64)> = None;
    let mut last_committed_transaction_sequence: u64 = 0;

    for (index, (instruction, entry)) in bytecode.instructions.iter().zip(trace.iter()).enumerate()
    {
        if entry.pre_state_hash != state_hash(&entry.pre_state) {
            return Err(format!(
                "FP4016 VERIFICATION_FAILED: pre-state witness hash mismatch at instruction {}.",
                index
            ));
        }

        if entry.post_state_hash != state_hash(&entry.post_state) {
            return Err(format!(
                "FP4017 VERIFICATION_FAILED: post-state witness hash mismatch at instruction {}.",
                index
            ));
        }

        if entry.pre_stack_hash != stack_hash(&entry.pre_stack) {
            return Err(format!(
                "FP4018 VERIFICATION_FAILED: pre-stack witness hash mismatch at instruction {}.",
                index
            ));
        }

        if entry.post_stack_hash != stack_hash(&entry.post_stack) {
            return Err(format!(
                "FP4019 VERIFICATION_FAILED: post-stack witness hash mismatch at instruction {}.",
                index
            ));
        }

        match instruction {
            crate::bytecode::BytecodeInstruction::InitAccount {
                name,
                account_type,
                currency,
                initial_balance,
            } => {
                if active_transaction.is_some() {
                    return Err(format!(
                        "FP4020 VERIFICATION_FAILED: InitAccount '{}' executed inside a transaction.",
                        name
                    ));
                }

                if entry.pre_state.accounts.contains_key(name) {
                    return Err(format!(
                        "FP4021 VERIFICATION_FAILED: InitAccount '{}' starts with an existing account.",
                        name
                    ));
                }

                if entry.pre_stack != entry.post_stack {
                    return Err(format!(
                        "FP4022 VERIFICATION_FAILED: InitAccount '{}' changed the stack.",
                        name
                    ));
                }

                if entry.pre_state.accounts.len() + 1 != entry.post_state.accounts.len() {
                    return Err(format!(
                        "FP4023 VERIFICATION_FAILED: InitAccount '{}' changed an unexpected number of accounts.",
                        name
                    ));
                }

                for (account_name, account) in &entry.pre_state.accounts {
                    let post_account = entry.post_state.accounts.get(account_name).ok_or_else(|| {
                        format!(
                            "FP4024 VERIFICATION_FAILED: InitAccount '{}' removed account '{}'.",
                            name, account_name
                        )
                    })?;

                    if post_account != account {
                        return Err(format!(
                            "FP4025 VERIFICATION_FAILED: InitAccount '{}' modified account '{}'.",
                            name, account_name
                        ));
                    }
                }

                let account = entry.post_state.accounts.get(name).ok_or_else(|| {
                    format!(
                        "FP4026 VERIFICATION_FAILED: InitAccount '{}' is missing from post-state.",
                        name
                    )
                })?;

                if account.account_type != *account_type
                    || account.currency != *currency
                    || account.balance != *initial_balance
                {
                    return Err(format!(
                        "FP4027 VERIFICATION_FAILED: InitAccount '{}' produced an invalid account state.",
                        name
                    ));
                }
            }

            crate::bytecode::BytecodeInstruction::BeginTransaction { name, sequence } => {
                if active_transaction.is_some() {
                    return Err(format!(
                        "FP4028 VERIFICATION_FAILED: nested transaction '{}' at instruction {}.",
                        name, index
                    ));
                }

                if *sequence == 0 {
                    return Err(format!(
                        "FP4029 VERIFICATION_FAILED: transaction '{}' has invalid sequence 0.",
                        name
                    ));
                }

                if *sequence != last_committed_transaction_sequence + 1 {
                    return Err(format!(
                        "FP4030 VERIFICATION_FAILED: transaction '{}' has invalid sequence {}.",
                        name, sequence
                    ));
                }

                if entry.pre_state != entry.post_state {
                    return Err(format!(
                        "FP4031 VERIFICATION_FAILED: BeginTransaction '{}' changed state.",
                        name
                    ));
                }

                if entry.pre_stack != entry.post_stack {
                    return Err(format!(
                        "FP4032 VERIFICATION_FAILED: BeginTransaction '{}' changed the stack.",
                        name
                    ));
                }

                active_transaction = Some((name.clone(), *sequence));
            }

            crate::bytecode::BytecodeInstruction::EndTransaction => {
                let (transaction_name, transaction_sequence) =
                    active_transaction.clone().ok_or_else(|| {
                        format!(
                            "FP4033 VERIFICATION_FAILED: EndTransaction without an active transaction at instruction {}.",
                            index
                        )
                    })?;

                if entry.pre_state != entry.post_state {
                    return Err(format!(
                        "FP4034 VERIFICATION_FAILED: EndTransaction for '{}' changed state.",
                        transaction_name
                    ));
                }

                if !entry.pre_stack.is_empty() || !entry.post_stack.is_empty() {
                    return Err(format!(
                        "FP4035 VERIFICATION_FAILED: EndTransaction for '{}' did not have an empty stack.",
                        transaction_name
                    ));
                }

                last_committed_transaction_sequence = transaction_sequence;
                active_transaction = None;
            }

            crate::bytecode::BytecodeInstruction::PushMoney { amount, currency } => {
                if entry.pre_state != entry.post_state {
                    return Err(format!(
                        "FP4036 VERIFICATION_FAILED: PushMoney changed state at instruction {}.",
                        index
                    ));
                }

                if entry.post_stack.len() != entry.pre_stack.len() + 1 {
                    return Err(format!(
                        "FP4037 VERIFICATION_FAILED: PushMoney changed stack size incorrectly at instruction {}.",
                        index
                    ));
                }

                if entry.post_stack[..entry.pre_stack.len()] != entry.pre_stack[..] {
                    return Err(format!(
                        "FP4038 VERIFICATION_FAILED: PushMoney changed the existing stack entries at instruction {}.",
                        index
                    ));
                }

                let pushed = entry.post_stack.last().ok_or_else(|| {
                    format!(
                        "FP4039 VERIFICATION_FAILED: PushMoney produced no stack value at instruction {}.",
                        index
                    )
                })?;

                if pushed.0 != *amount || pushed.1 != *currency {
                    return Err(format!(
                        "FP4040 VERIFICATION_FAILED: PushMoney produced an incorrect stack value at instruction {}.",
                        index
                    ));
                }
            }

            crate::bytecode::BytecodeInstruction::Add => {
                if entry.pre_state != entry.post_state {
                    return Err(format!(
                        "FP4041 VERIFICATION_FAILED: Add changed state at instruction {}.",
                        index
                    ));
                }

                if entry.pre_stack.len() < 2 {
                    return Err(format!(
                        "FP4042 VERIFICATION_FAILED: Add requires two stack values at instruction {}.",
                        index
                    ));
                }

                let left = &entry.pre_stack[entry.pre_stack.len() - 2];
                let right = &entry.pre_stack[entry.pre_stack.len() - 1];

                if left.1 != right.1 {
                    return Err(format!(
                        "FP4043 VERIFICATION_FAILED: Add encountered currency mismatch at instruction {}.",
                        index
                    ));
                }

                let result = left.0.checked_add(right.0).map_err(|_| {
                    format!(
                        "FP4044 VERIFICATION_FAILED: Add overflow at instruction {}.",
                        index
                    )
                })?;

                let expected_len = entry.pre_stack.len() - 1;

                if entry.post_stack.len() != expected_len {
                    return Err(format!(
                        "FP4045 VERIFICATION_FAILED: Add produced incorrect stack size at instruction {}.",
                        index
                    ));
                }

                let prefix_len = entry.pre_stack.len() - 2;
                if entry.post_stack[..prefix_len] != entry.pre_stack[..prefix_len] {
                    return Err(format!(
                        "FP4046 VERIFICATION_FAILED: Add changed unrelated stack entries at instruction {}.",
                        index
                    ));
                }

                let pushed = entry.post_stack.last().ok_or_else(|| {
                    format!(
                        "FP4047 VERIFICATION_FAILED: Add produced no result at instruction {}.",
                        index
                    )
                })?;

                if pushed.0 != result || pushed.1 != left.1 {
                    return Err(format!(
                        "FP4048 VERIFICATION_FAILED: Add produced an incorrect result at instruction {}.",
                        index
                    ));
                }
            }

            crate::bytecode::BytecodeInstruction::Subtract => {
                if entry.pre_state != entry.post_state {
                    return Err(format!(
                        "FP4049 VERIFICATION_FAILED: Subtract changed state at instruction {}.",
                        index
                    ));
                }

                if entry.pre_stack.len() < 2 {
                    return Err(format!(
                        "FP4050 VERIFICATION_FAILED: Subtract requires two stack values at instruction {}.",
                        index
                    ));
                }

                let left = &entry.pre_stack[entry.pre_stack.len() - 2];
                let right = &entry.pre_stack[entry.pre_stack.len() - 1];

                if left.1 != right.1 {
                    return Err(format!(
                        "FP4051 VERIFICATION_FAILED: Subtract encountered currency mismatch at instruction {}.",
                        index
                    ));
                }

                let result = left.0.checked_sub(right.0).map_err(|_| {
                    format!(
                        "FP4052 VERIFICATION_FAILED: Subtract overflow at instruction {}.",
                        index
                    )
                })?;

                if entry.post_stack.len() != entry.pre_stack.len() - 1 {
                    return Err(format!(
                        "FP4053 VERIFICATION_FAILED: Subtract produced incorrect stack size at instruction {}.",
                        index
                    ));
                }

                let prefix_len = entry.pre_stack.len() - 2;
                if entry.post_stack[..prefix_len] != entry.pre_stack[..prefix_len] {
                    return Err(format!(
                        "FP4054 VERIFICATION_FAILED: Subtract changed unrelated stack entries at instruction {}.",
                        index
                    ));
                }

                let pushed = entry.post_stack.last().ok_or_else(|| {
                    format!(
                        "FP4055 VERIFICATION_FAILED: Subtract produced no result at instruction {}.",
                        index
                    )
                })?;

                if pushed.0 != result || pushed.1 != left.1 {
                    return Err(format!(
                        "FP4056 VERIFICATION_FAILED: Subtract produced an incorrect result at instruction {}.",
                        index
                    ));
                }
            }

            crate::bytecode::BytecodeInstruction::Pay { from, to }
            | crate::bytecode::BytecodeInstruction::Transfer { from, to } => {
                let operation = match instruction {
                    crate::bytecode::BytecodeInstruction::Pay { .. } => "Pay",
                    crate::bytecode::BytecodeInstruction::Transfer { .. } => "Transfer",
                    _ => unreachable!(),
                };

                if active_transaction.is_none() {
                    return Err(format!(
                        "FP4057 VERIFICATION_FAILED: {} executed outside a transaction at instruction {}.",
                        operation, index
                    ));
                }

                if from == to {
                    return Err(format!(
                        "FP4058 VERIFICATION_FAILED: {} cannot transfer to the same account at instruction {}.",
                        operation, index
                    ));
                }

                if entry.pre_stack.is_empty() {
                    return Err(format!(
                        "FP4059 VERIFICATION_FAILED: {} requires a stack value at instruction {}.",
                        operation, index
                    ));
                }

                if entry.post_stack.len() + 1 != entry.pre_stack.len() {
                    return Err(format!(
                        "FP4060 VERIFICATION_FAILED: {} produced incorrect stack size at instruction {}.",
                        operation, index
                    ));
                }

                if entry.post_stack != entry.pre_stack[..entry.pre_stack.len() - 1] {
                    return Err(format!(
                        "FP4061 VERIFICATION_FAILED: {} changed unrelated stack entries at instruction {}.",
                        operation, index
                    ));
                }

                let value = entry.pre_stack.last().unwrap();

                let from_account = entry.pre_state.accounts.get(from).ok_or_else(|| {
                    format!(
                        "FP4062 VERIFICATION_FAILED: {} source account '{}' does not exist.",
                        operation, from
                    )
                })?;

                let to_account = entry.pre_state.accounts.get(to).ok_or_else(|| {
                    format!(
                        "FP4063 VERIFICATION_FAILED: {} destination account '{}' does not exist.",
                        operation, to
                    )
                })?;

                if from_account.currency != to_account.currency || from_account.currency != value.1
                {
                    return Err(format!(
                        "FP4064 VERIFICATION_FAILED: {} currency mismatch at instruction {}.",
                        operation, index
                    ));
                }

                if value.0.minor_units() <= 0 {
                    return Err(format!(
                        "FP4065 VERIFICATION_FAILED: {} amount must be positive at instruction {}.",
                        operation, index
                    ));
                }

                if from_account.balance.minor_units() < value.0.minor_units() {
                    return Err(format!(
                        "FP4066 VERIFICATION_FAILED: {} has insufficient funds in '{}' at instruction {}.",
                        operation, from, index
                    ));
                }

                let expected_from = from_account.balance.checked_sub(value.0).map_err(|_| {
                    format!(
                        "FP4067 VERIFICATION_FAILED: {} source balance overflow at instruction {}.",
                        operation, index
                    )
                })?;

                let expected_to = to_account.balance.checked_add(value.0).map_err(|_| {
                    format!(
                        "FP4068 VERIFICATION_FAILED: {} destination balance overflow at instruction {}.",
                        operation, index
                    )
                })?;

                let mut expected_state = entry.pre_state.clone();

                expected_state.accounts.get_mut(from).unwrap().balance = expected_from;

                expected_state.accounts.get_mut(to).unwrap().balance = expected_to;

                if entry.post_state != expected_state {
                    return Err(format!(
                        "FP4069 VERIFICATION_FAILED: {} produced an invalid state transition at instruction {}.",
                        operation, index
                    ));
                }
            }

            crate::bytecode::BytecodeInstruction::Debit { account } => {
                if active_transaction.is_none() {
                    return Err(format!(
                        "FP4070 VERIFICATION_FAILED: Debit executed outside a transaction at instruction {}.",
                        index
                    ));
                }

                if entry.pre_stack.is_empty() {
                    return Err(format!(
                        "FP4071 VERIFICATION_FAILED: Debit requires a stack value at instruction {}.",
                        index
                    ));
                }

                if entry.post_stack.len() + 1 != entry.pre_stack.len()
                    || entry.post_stack != entry.pre_stack[..entry.pre_stack.len() - 1]
                {
                    return Err(format!(
                        "FP4072 VERIFICATION_FAILED: Debit produced an invalid stack transition at instruction {}.",
                        index
                    ));
                }

                let value = entry.pre_stack.last().unwrap();

                if value.0.minor_units() <= 0 {
                    return Err(format!(
                        "FP4073 VERIFICATION_FAILED: Debit amount must be positive at instruction {}.",
                        index
                    ));
                }

                let account_state = entry.pre_state.accounts.get(account).ok_or_else(|| {
                    format!(
                        "FP4074 VERIFICATION_FAILED: Debit account '{}' does not exist.",
                        account
                    )
                })?;

                if account_state.currency != value.1 {
                    return Err(format!(
                        "FP4075 VERIFICATION_FAILED: Debit currency mismatch at instruction {}.",
                        index
                    ));
                }

                let increases = matches!(
                    account_state.account_type,
                    crate::ast::AccountType::Asset | crate::ast::AccountType::Expense
                );

                let expected_balance = if increases {
                    account_state.balance.checked_add(value.0).map_err(|_| {
                        format!(
                            "FP4076 VERIFICATION_FAILED: Debit balance overflow at instruction {}.",
                            index
                        )
                    })?
                } else {
                    if account_state.balance.minor_units() < value.0.minor_units() {
                        return Err(format!(
                            "FP4077 VERIFICATION_FAILED: Debit has insufficient funds in '{}' at instruction {}.",
                            account, index
                        ));
                    }

                    account_state.balance.checked_sub(value.0).map_err(|_| {
                        format!(
                            "FP4078 VERIFICATION_FAILED: Debit balance overflow at instruction {}.",
                            index
                        )
                    })?
                };

                let mut expected_state = entry.pre_state.clone();
                expected_state.accounts.get_mut(account).unwrap().balance = expected_balance;

                if entry.post_state != expected_state {
                    return Err(format!(
                        "FP4079 VERIFICATION_FAILED: Debit produced an invalid state transition at instruction {}.",
                        index
                    ));
                }
            }

            crate::bytecode::BytecodeInstruction::Credit { account } => {
                if active_transaction.is_none() {
                    return Err(format!(
                        "FP4080 VERIFICATION_FAILED: Credit executed outside a transaction at instruction {}.",
                        index
                    ));
                }

                if entry.pre_stack.is_empty() {
                    return Err(format!(
                        "FP4081 VERIFICATION_FAILED: Credit requires a stack value at instruction {}.",
                        index
                    ));
                }

                if entry.post_stack.len() + 1 != entry.pre_stack.len()
                    || entry.post_stack != entry.pre_stack[..entry.pre_stack.len() - 1]
                {
                    return Err(format!(
                        "FP4082 VERIFICATION_FAILED: Credit produced an invalid stack transition at instruction {}.",
                        index
                    ));
                }

                let value = entry.pre_stack.last().unwrap();

                if value.0.minor_units() <= 0 {
                    return Err(format!(
                        "FP4083 VERIFICATION_FAILED: Credit amount must be positive at instruction {}.",
                        index
                    ));
                }

                let account_state = entry.pre_state.accounts.get(account).ok_or_else(|| {
                    format!(
                        "FP4084 VERIFICATION_FAILED: Credit account '{}' does not exist.",
                        account
                    )
                })?;

                if account_state.currency != value.1 {
                    return Err(format!(
                        "FP4085 VERIFICATION_FAILED: Credit currency mismatch at instruction {}.",
                        index
                    ));
                }

                let increases = matches!(
                    account_state.account_type,
                    crate::ast::AccountType::Liability
                        | crate::ast::AccountType::Equity
                        | crate::ast::AccountType::Revenue
                );

                let expected_balance = if increases {
                    account_state.balance.checked_add(value.0).map_err(|_| {
                        format!(
                            "FP4086 VERIFICATION_FAILED: Credit balance overflow at instruction {}.",
                            index
                        )
                    })?
                } else {
                    if account_state.balance.minor_units() < value.0.minor_units() {
                        return Err(format!(
                            "FP4087 VERIFICATION_FAILED: Credit has insufficient funds in '{}' at instruction {}.",
                            account, index
                        ));
                    }

                    account_state.balance.checked_sub(value.0).map_err(|_| {
                        format!(
                            "FP4088 VERIFICATION_FAILED: Credit balance overflow at instruction {}.",
                            index
                        )
                    })?
                };

                let mut expected_state = entry.pre_state.clone();
                expected_state.accounts.get_mut(account).unwrap().balance = expected_balance;

                if entry.post_state != expected_state {
                    return Err(format!(
                        "FP4089 VERIFICATION_FAILED: Credit produced an invalid state transition at instruction {}.",
                        index
                    ));
                }
            }
        }
    }

    if active_transaction.is_some() {
        return Err(
            "FP4090 VERIFICATION_FAILED: execution trace ended with an active transaction."
                .to_string(),
        );
    }

    Ok(())
}

#[allow(dead_code)]
pub struct VerificationCommitment {
    pub program_hash: [u8; 32],
    pub bytecode_hash: [u8; 32],
    pub state_hash: [u8; 32],
    pub transaction_hashes: Vec<[u8; 32]>,
    pub trace_root: [u8; 32],
}

#[allow(dead_code)]
impl VerificationCommitment {
    pub fn from_execution(
        program: &crate::ast::Program,
        bytecode: &BytecodeProgram,
        result: &crate::runtime::ExecutionResult,
    ) -> Self {
        let transaction_hashes = result
            .ledger
            .iter()
            .map(transaction_hash)
            .collect::<Vec<_>>();

        Self {
            program_hash: program_hash(program),
            bytecode_hash: bytecode_hash(bytecode),
            state_hash: state_hash(&result.state),
            transaction_hashes,
            trace_root: trace_root(&result.trace),
        }
    }

    pub fn canonical_representation(&self) -> String {
        format!(
            "program_hash={};bytecode_hash={};state_hash={};transaction_hashes={};trace_root={}",
            hex_encode(&self.program_hash),
            hex_encode(&self.bytecode_hash),
            hex_encode(&self.state_hash),
            self.transaction_hashes
                .iter()
                .map(hex_encode)
                .collect::<Vec<_>>()
                .join(","),
            hex_encode(&self.trace_root),
        )
    }

    pub fn verify_execution(
        &self,
        program: &crate::ast::Program,
        bytecode: &BytecodeProgram,
        result: &crate::runtime::ExecutionResult,
    ) -> Result<(), String> {
        let expected = VerificationCommitment::from_execution(program, bytecode, result);

        if self.program_hash != expected.program_hash {
            return Err("FP4001 VERIFICATION_FAILED: program hash mismatch.".to_string());
        }

        if self.bytecode_hash != expected.bytecode_hash {
            return Err("FP4002 VERIFICATION_FAILED: bytecode hash mismatch.".to_string());
        }
        if self.state_hash != expected.state_hash {
            return Err("FP4003 VERIFICATION_FAILED: state hash mismatch.".to_string());
        }

        if self.transaction_hashes != expected.transaction_hashes {
            return Err("FP4004 VERIFICATION_FAILED: transaction commitment mismatch.".to_string());
        }

        if self.trace_root != expected.trace_root {
            return Err("FP4005 VERIFICATION_FAILED: trace root mismatch.".to_string());
        }
        let independently_executed = crate::vm::execute_bytecode(bytecode).map_err(|error| {
            format!(
                "FP4011 VERIFICATION_FAILED: independent execution failed: {}.",
                error
            )
        })?;

        if independently_executed.state != result.state {
            return Err(
                "FP4012 VERIFICATION_FAILED: independently executed state mismatch.".to_string(),
            );
        }

        if independently_executed.ledger != result.ledger {
            return Err(
                "FP4013 VERIFICATION_FAILED: independently executed ledger mismatch.".to_string(),
            );
        }

        if independently_executed.trace != result.trace {
            return Err(
                "FP4014 VERIFICATION_FAILED: independently executed trace mismatch.".to_string(),
            );
        }
        verify_trace_matches_bytecode(&result.trace, bytecode)?;
        verify_trace_continuity(&result.trace)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::BytecodeInstruction;
    use crate::types::MoneyAmount;

    #[test]
    fn verification_commitment_matches_execution() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        assert_eq!(commitment.program_hash, program_hash(&program));
        assert_eq!(commitment.bytecode_hash, bytecode_hash(&bytecode));
        assert_eq!(commitment.state_hash, state_hash(&result.state));
        assert_eq!(commitment.trace_root, trace_root(&result.trace));

        assert!(
            commitment
                .verify_execution(&program, &bytecode, &result)
                .is_ok()
        );
        assert_eq!(commitment.transaction_hashes.len(), result.ledger.len());
    }
    #[test]
    fn verification_commitment_rejects_tampered_program_hash() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        commitment.program_hash[0] ^= 1;

        let error = commitment
            .verify_execution(&program, &bytecode, &result)
            .unwrap_err();

        assert_eq!(error, "FP4001 VERIFICATION_FAILED: program hash mismatch.");
    }
    #[test]
    fn verification_commitment_rejects_tampered_bytecode_hash() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        commitment.bytecode_hash[0] ^= 1;

        let error = commitment
            .verify_execution(&program, &bytecode, &result)
            .unwrap_err();

        assert_eq!(error, "FP4002 VERIFICATION_FAILED: bytecode hash mismatch.");
    }
    #[test]
    fn verification_commitment_rejects_tampered_state_hash() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        commitment.state_hash[0] ^= 1;

        let error = commitment
            .verify_execution(&program, &bytecode, &result)
            .unwrap_err();

        assert_eq!(error, "FP4003 VERIFICATION_FAILED: state hash mismatch.");
    }
    #[test]
    fn verification_commitment_rejects_tampered_transaction_commitment() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        commitment.transaction_hashes[0][0] ^= 1;

        let error = commitment
            .verify_execution(&program, &bytecode, &result)
            .unwrap_err();

        assert_eq!(
            error,
            "FP4004 VERIFICATION_FAILED: transaction commitment mismatch."
        );
    }
    #[test]
    fn verification_commitment_rejects_tampered_trace_root() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        commitment.trace_root[0] ^= 1;

        let error = commitment
            .verify_execution(&program, &bytecode, &result)
            .unwrap_err();

        assert_eq!(error, "FP4005 VERIFICATION_FAILED: trace root mismatch.");
    }
    #[test]
    fn verification_commitment_rejects_different_execution_result() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        let mut tampered_result = result.clone();
        tampered_result
            .state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_minor_units(6100);

        let error = commitment
            .verify_execution(&program, &bytecode, &tampered_result)
            .unwrap_err();

        assert_eq!(error, "FP4003 VERIFICATION_FAILED: state hash mismatch.");
    }
    #[test]
    fn verification_commitment_rejects_tampered_execution_trace() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        assert!(!result.trace.is_empty());

        let commitment = VerificationCommitment::from_execution(&program, &bytecode, &result);

        let mut tampered_result = result.clone();
        tampered_result.trace[0].instruction.push_str("|tampered");

        let error = commitment
            .verify_execution(&program, &bytecode, &tampered_result)
            .unwrap_err();

        assert_eq!(error, "FP4005 VERIFICATION_FAILED: trace root mismatch.");
    }
    #[test]
    fn verification_commitment_canonical_representation_is_deterministic() {
        let commitment = VerificationCommitment {
            program_hash: [1u8; 32],
            bytecode_hash: [2u8; 32],
            state_hash: [3u8; 32],
            transaction_hashes: vec![[4u8; 32], [5u8; 32]],
            trace_root: [6u8; 32],
        };

        let first = commitment.canonical_representation();
        let second = commitment.canonical_representation();

        assert_eq!(first, second);
        assert!(first.contains("program_hash="));
        assert!(first.contains("bytecode_hash="));
        assert!(first.contains("state_hash="));
        assert!(first.contains("transaction_hashes="));
        assert!(first.contains("trace_root="));
    }

    #[test]
    fn program_hash_hex_has_64_characters() {
        let program = crate::ast::Program {
            declarations: vec![crate::ast::Declaration::Currency(
                crate::ast::CurrencyDeclaration {
                    code: "USD".to_string(),
                },
            )],
        };

        let hash = program_hash_hex(&program);

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn program_hash_is_deterministic() {
        let program = crate::ast::Program {
            declarations: vec![crate::ast::Declaration::Currency(
                crate::ast::CurrencyDeclaration {
                    code: "USD".to_string(),
                },
            )],
        };

        let first = program_hash(&program);
        let second = program_hash(&program);

        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn equivalent_money_formats_produce_same_program_hash() {
        let first = crate::ast::Program {
            declarations: vec![
                crate::ast::Declaration::Currency(crate::ast::CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                crate::ast::Declaration::Account(crate::ast::AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: crate::ast::AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_decimal_str("100").unwrap(),
                }),
            ],
        };

        let second = crate::ast::Program {
            declarations: vec![
                crate::ast::Declaration::Currency(crate::ast::CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                crate::ast::Declaration::Account(crate::ast::AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: crate::ast::AccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: crate::types::MoneyAmount::from_decimal_str("100.00").unwrap(),
                }),
            ],
        };

        assert_eq!(program_hash(&first), program_hash(&second));
    }
    #[test]
    fn trace_root_is_deterministic() {
        let trace = vec![
            ExecutionTraceEntry {
                step: 1,
                instruction: "PushMoney|amount_minor_units=10000|currency=USD".to_string(),
                pre_state_hash: [0u8; 32],
                post_state_hash: [1u8; 32],
                pre_stack_hash: [2u8; 32],
                post_stack_hash: [3u8; 32],
                ..Default::default()
            },
            ExecutionTraceEntry {
                step: 2,
                instruction: "Add".to_string(),
                pre_state_hash: [1u8; 32],
                post_state_hash: [4u8; 32],
                pre_stack_hash: [3u8; 32],
                post_stack_hash: [5u8; 32],
                ..Default::default()
            },
        ];

        assert_eq!(trace_root(&trace), trace_root(&trace));
        assert_eq!(trace_root(&trace).len(), 32);
    }

    #[test]
    fn changing_trace_entry_changes_trace_root() {
        let first = vec![ExecutionTraceEntry {
            step: 1,
            instruction: "PushMoney|amount_minor_units=10000|currency=USD".to_string(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        }];

        let mut second = first.clone();
        second[0].step = 2;

        assert_ne!(trace_root(&first), trace_root(&second));
    }

    #[test]
    fn changing_trace_order_changes_trace_root() {
        let first_entry = ExecutionTraceEntry {
            step: 1,
            instruction: "PushMoney|amount_minor_units=10000|currency=USD".to_string(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        };

        let second_entry = ExecutionTraceEntry {
            step: 2,
            instruction: "Add".to_string(),
            pre_state_hash: [1u8; 32],
            post_state_hash: [4u8; 32],
            pre_stack_hash: [3u8; 32],
            post_stack_hash: [5u8; 32],
            ..Default::default()
        };

        let first = vec![first_entry.clone(), second_entry.clone()];
        let second = vec![second_entry, first_entry];

        assert_ne!(trace_root(&first), trace_root(&second));
    }

    #[test]
    fn trace_root_hex_has_64_characters() {
        let trace = vec![ExecutionTraceEntry {
            step: 1,
            instruction: "Add".to_string(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        }];

        let root = trace_root_hex(&trace);

        assert_eq!(root.len(), 64);
        assert!(root.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn execution_trace_entry_canonical_representation_is_deterministic() {
        let entry = ExecutionTraceEntry {
            step: 1,
            instruction: "PushMoney|amount_minor_units=10000|currency=USD".to_string(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        };

        let first = entry.canonical_representation();
        let second = entry.canonical_representation();

        assert_eq!(first, second);
    }

    #[test]
    fn execution_trace_entry_changes_when_step_changes() {
        let first = ExecutionTraceEntry {
            step: 1,
            instruction: "PushMoney|amount_minor_units=10000|currency=USD".to_string(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        };

        let mut second = first.clone();
        second.step = 2;

        assert_ne!(
            first.canonical_representation(),
            second.canonical_representation()
        );
    }
    #[test]
    fn verify_trace_continuity_accepts_valid_trace() {
        let trace = vec![
            ExecutionTraceEntry {
                step: 1,
                instruction: "PushMoney".to_string(),
                pre_state_hash: [0u8; 32],
                post_state_hash: [1u8; 32],
                pre_stack_hash: [2u8; 32],
                post_stack_hash: [3u8; 32],
                ..Default::default()
            },
            ExecutionTraceEntry {
                step: 2,
                instruction: "Pay".to_string(),
                pre_state_hash: [1u8; 32],
                post_state_hash: [4u8; 32],
                pre_stack_hash: [3u8; 32],
                post_stack_hash: [5u8; 32],
                ..Default::default()
            },
        ];

        assert!(verify_trace_continuity(&trace).is_ok());
    }
    #[test]
    fn transaction_hash_is_deterministic() {
        let entry = crate::runtime::LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: MoneyAmount::from_decimal_str("500.00").unwrap(),
            from_after: MoneyAmount::from_decimal_str("400.00").unwrap(),
            to_before: MoneyAmount::from_decimal_str("0.00").unwrap(),
            to_after: MoneyAmount::from_decimal_str("100.00").unwrap(),
        };

        assert_eq!(transaction_hash(&entry), transaction_hash(&entry));
    }

    #[test]
    fn different_transaction_amount_produces_different_hash() {
        let mut first = crate::runtime::LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: MoneyAmount::from_decimal_str("500.00").unwrap(),
            from_after: MoneyAmount::from_decimal_str("400.00").unwrap(),
            to_before: MoneyAmount::from_decimal_str("0.00").unwrap(),
            to_after: MoneyAmount::from_decimal_str("100.00").unwrap(),
        };

        let mut second = first.clone();
        second.amount = MoneyAmount::from_decimal_str("200.00").unwrap();

        assert_ne!(transaction_hash(&first), transaction_hash(&second));

        first.amount = MoneyAmount::from_decimal_str("100.00").unwrap();
    }

    #[test]
    fn different_transaction_account_produces_different_hash() {
        let first = crate::runtime::LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: MoneyAmount::from_decimal_str("500.00").unwrap(),
            from_after: MoneyAmount::from_decimal_str("400.00").unwrap(),
            to_before: MoneyAmount::from_decimal_str("0.00").unwrap(),
            to_after: MoneyAmount::from_decimal_str("100.00").unwrap(),
        };

        let mut second = first.clone();
        second.to = "Bank".to_string();

        assert_ne!(transaction_hash(&first), transaction_hash(&second));
    }

    #[test]
    fn transaction_hash_hex_has_64_characters() {
        let entry = crate::runtime::LedgerEntry {
            sequence: 1,
            logical_time: 1,
            transaction_sequence: 1,
            transaction: "Sale".to_string(),
            operation: "pay".to_string(),
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
            from_before: MoneyAmount::from_decimal_str("500.00").unwrap(),
            from_after: MoneyAmount::from_decimal_str("400.00").unwrap(),
            to_before: MoneyAmount::from_decimal_str("0.00").unwrap(),
            to_after: MoneyAmount::from_decimal_str("100.00").unwrap(),
        };

        let hash = transaction_hash_hex(&entry);

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn state_hash_is_deterministic() {
        let mut accounts = std::collections::HashMap::new();

        accounts.insert(
            "Cash".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("100.00").unwrap(),
            },
        );

        let state = crate::runtime::ExecutionState { accounts };

        let first = state_hash(&state);
        let second = state_hash(&state);

        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn different_state_produces_different_hash() {
        let mut first_accounts = std::collections::HashMap::new();

        first_accounts.insert(
            "Cash".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("100.00").unwrap(),
            },
        );

        let mut second_accounts = std::collections::HashMap::new();

        second_accounts.insert(
            "Cash".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("101.00").unwrap(),
            },
        );

        let first = crate::runtime::ExecutionState {
            accounts: first_accounts,
        };

        let second = crate::runtime::ExecutionState {
            accounts: second_accounts,
        };

        assert_ne!(state_hash(&first), state_hash(&second));
    }

    #[test]
    fn state_hash_is_independent_of_hashmap_insertion_order() {
        let cash = crate::runtime::AccountState {
            account_type: crate::ast::AccountType::Asset,
            currency: "USD".to_string(),
            balance: MoneyAmount::from_decimal_str("100.00").unwrap(),
        };

        let bank = crate::runtime::AccountState {
            account_type: crate::ast::AccountType::Asset,
            currency: "USD".to_string(),
            balance: MoneyAmount::from_decimal_str("200.00").unwrap(),
        };

        let mut first_accounts = std::collections::HashMap::new();
        first_accounts.insert("Cash".to_string(), cash.clone());
        first_accounts.insert("Bank".to_string(), bank.clone());

        let mut second_accounts = std::collections::HashMap::new();
        second_accounts.insert("Bank".to_string(), bank);
        second_accounts.insert("Cash".to_string(), cash);

        let first = crate::runtime::ExecutionState {
            accounts: first_accounts,
        };

        let second = crate::runtime::ExecutionState {
            accounts: second_accounts,
        };

        assert_eq!(state_hash(&first), state_hash(&second));
    }

    #[test]
    fn state_hash_hex_has_64_characters() {
        let state = crate::runtime::ExecutionState {
            accounts: std::collections::HashMap::new(),
        };

        let hash = state_hash_hex(&state);

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn bytecode_hash_is_deterministic() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Add,
        ]);

        let first = bytecode_hash(&program);
        let second = bytecode_hash(&program);

        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn bytecode_hash_hex_has_64_characters() {
        let program = BytecodeProgram::new(vec![BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
        }]);

        let hash = bytecode_hash_hex(&program);

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn different_bytecode_produces_different_hash() {
        let first = BytecodeProgram::new(vec![BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
        }]);

        let second = BytecodeProgram::new(vec![BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_decimal_str("200.00").unwrap(),
            currency: "USD".to_string(),
        }]);

        assert_ne!(bytecode_hash(&first), bytecode_hash(&second));
    }
    #[test]
    fn verify_trace_matches_bytecode_accepts_matching_trace() {
        let bytecode = BytecodeProgram::new(vec![
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Add,
        ]);

        let trace = vec![
            ExecutionTraceEntry {
                step: 1,
                instruction: bytecode.instructions[0].canonical_representation(),
                pre_state_hash: [0u8; 32],
                post_state_hash: [1u8; 32],
                pre_stack_hash: [2u8; 32],
                post_stack_hash: [3u8; 32],
                ..Default::default()
            },
            ExecutionTraceEntry {
                step: 2,
                instruction: bytecode.instructions[1].canonical_representation(),
                pre_state_hash: [1u8; 32],
                post_state_hash: [4u8; 32],
                pre_stack_hash: [3u8; 32],
                post_stack_hash: [5u8; 32],
                ..Default::default()
            },
        ];

        assert!(verify_trace_matches_bytecode(&trace, &bytecode).is_ok());
    }

    #[test]
    fn verify_trace_matches_bytecode_rejects_length_mismatch() {
        let bytecode = BytecodeProgram::new(vec![
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Add,
        ]);

        let trace = vec![ExecutionTraceEntry {
            step: 1,
            instruction: bytecode.instructions[0].canonical_representation(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        }];

        let error = verify_trace_matches_bytecode(&trace, &bytecode).unwrap_err();

        assert!(error.starts_with("FP4009 VERIFICATION_FAILED"));
    }

    #[test]
    fn verify_trace_matches_bytecode_rejects_instruction_mismatch() {
        let bytecode = BytecodeProgram::new(vec![BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
            currency: "USD".to_string(),
        }]);

        let trace = vec![ExecutionTraceEntry {
            step: 1,
            instruction: "Add".to_string(),
            pre_state_hash: [0u8; 32],
            post_state_hash: [1u8; 32],
            pre_stack_hash: [2u8; 32],
            post_stack_hash: [3u8; 32],
            ..Default::default()
        }];

        let error = verify_trace_matches_bytecode(&trace, &bytecode).unwrap_err();

        assert!(error.starts_with("FP4010 VERIFICATION_FAILED"));
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_instruction() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Debit|"))
            .unwrap();

        trace[index].instruction = "Credit|account=Cash".to_string();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4010"),
            "expected instruction mismatch error, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_state_discontinuity() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        trace[4].pre_state_hash = [0u8; 32];

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4007"),
            "expected state transition discontinuity, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_stack_discontinuity() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        trace[4].pre_stack_hash = [0u8; 32];

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4008"),
            "expected stack transition discontinuity, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_step() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        trace[3].step = 99;

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4006"),
            "expected trace step discontinuity error, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_wrong_length() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        trace.pop();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4015"),
            "expected witness length mismatch, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_unclosed_transaction() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        bytecode.instructions.pop();
        let mut trace = result.trace.clone();
        trace.pop();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4090"),
            "expected unclosed transaction, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_nested_transaction() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::EndTransaction
                )
            })
            .unwrap();

        bytecode.instructions[index] = crate::bytecode::BytecodeInstruction::BeginTransaction {
            name: "Nested".to_string(),
            sequence: 2,
        };

        trace[index].instruction = bytecode.instructions[index].canonical_representation();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4028"),
            "expected nested-transaction rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_begin_transaction_zero_sequence() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::BeginTransaction { .. }
                )
            })
            .unwrap();

        bytecode.instructions[index] = crate::bytecode::BytecodeInstruction::BeginTransaction {
            name: "Sale".to_string(),
            sequence: 0,
        };

        trace[index].instruction = bytecode.instructions[index].canonical_representation();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4029"),
            "expected zero-sequence rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_begin_transaction_invalid_sequence() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::BeginTransaction { .. }
                )
            })
            .unwrap();

        bytecode.instructions[index] = crate::bytecode::BytecodeInstruction::BeginTransaction {
            name: "Sale".to_string(),
            sequence: 2,
        };

        trace[index].instruction = bytecode.instructions[index].canonical_representation();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4030"),
            "expected invalid-sequence rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_begin_transaction_state_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("BeginTransaction|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("99.00").unwrap();
        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4031"),
            "expected BeginTransaction state-change rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_begin_transaction_stack_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("BeginTransaction|"))
            .unwrap();

        trace[index].post_stack.push((
            MoneyAmount::from_decimal_str("1.00").unwrap(),
            "USD".to_string(),
        ));
        trace[index].post_stack_hash = crate::verification::stack_hash(&trace[index].post_stack);

        trace[index + 1].pre_stack = trace[index].post_stack.clone();
        trace[index + 1].pre_stack_hash =
            crate::verification::stack_hash(&trace[index + 1].pre_stack);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4032"),
            "expected BeginTransaction stack-change rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_end_transaction_without_active_transaction() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::BeginTransaction { .. }
                )
            })
            .unwrap();

        bytecode.instructions[index] = crate::bytecode::BytecodeInstruction::EndTransaction;

        trace[index].instruction = bytecode.instructions[index].canonical_representation();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4033"),
            "expected EndTransaction without active transaction, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_end_transaction_state_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = trace
            .iter()
            .position(|entry| entry.instruction == "EndTransaction")
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("59.00").unwrap();
        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4034"),
            "expected EndTransaction state-change rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_end_transaction_nonempty_stack() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();
        let mut trace = result.trace.clone();

        let index = trace
            .iter()
            .position(|entry| entry.instruction == "EndTransaction")
            .unwrap();

        trace[index].post_stack.push((
            MoneyAmount::from_decimal_str("1.00").unwrap(),
            "USD".to_string(),
        ));
        trace[index].post_stack_hash = crate::verification::stack_hash(&trace[index].post_stack);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4035"),
            "expected EndTransaction non-empty-stack rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_init_account_inside_transaction() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::Debit { .. }
                )
            })
            .unwrap();

        bytecode.instructions[index] = crate::bytecode::BytecodeInstruction::InitAccount {
            name: "Fake".to_string(),
            account_type: crate::ast::AccountType::Asset,
            currency: "USD".to_string(),
            initial_balance: MoneyAmount::from_decimal_str("0.00").unwrap(),
        };

        trace[index].instruction = bytecode.instructions[index].canonical_representation();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4020"),
            "expected InitAccount inside transaction, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_init_account_existing_account() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let mut bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .nth(1)
            .map(|(index, _)| index)
            .unwrap();

        let mut trace = result.trace.clone();

        let (account_type, currency, initial_balance) = match &bytecode.instructions[index] {
            crate::bytecode::BytecodeInstruction::InitAccount {
                account_type,
                currency,
                initial_balance,
                ..
            } => (account_type.clone(), currency.clone(), *initial_balance),
            _ => panic!("expected InitAccount"),
        };

        bytecode.instructions[index] = crate::bytecode::BytecodeInstruction::InitAccount {
            name: "Cash".to_string(),
            account_type,
            currency,
            initial_balance,
        };

        trace[index].instruction = bytecode.instructions[index].canonical_representation();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4021"),
            "expected existing account rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_init_account_stack_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .nth(1)
            .map(|(index, _)| index)
            .unwrap();

        trace[index].post_stack.push((
            MoneyAmount::from_decimal_str("1.00").unwrap(),
            "USD".to_string(),
        ));
        trace[index].post_stack_hash = crate::verification::stack_hash(&trace[index].post_stack);

        trace[index + 1].pre_stack = trace[index].post_stack.clone();
        trace[index + 1].pre_stack_hash =
            crate::verification::stack_hash(&trace[index + 1].pre_stack);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4022"),
            "expected InitAccount stack change, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_init_account_count_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .nth(1)
            .map(|(index, _)| index)
            .unwrap();

        let extra_account = crate::runtime::AccountState {
            account_type: crate::ast::AccountType::Asset,
            currency: "USD".to_string(),
            balance: MoneyAmount::from_decimal_str("0.00").unwrap(),
        };

        trace[index]
            .post_state
            .accounts
            .insert("Fake".to_string(), extra_account);

        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4023"),
            "expected InitAccount account-count mismatch, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_init_account_removed_account() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .nth(1)
            .map(|(index, _)| index)
            .unwrap();

        trace[index].post_state.accounts.remove("Cash");

        trace[index].post_state.accounts.insert(
            "Fake".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("0.00").unwrap(),
            },
        );

        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4024"),
            "expected InitAccount removed-account rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_init_account_invalid_account_state() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .find(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .map(|(index, _)| index)
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("99.00").unwrap();

        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);

        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4027"),
            "expected InitAccount invalid-state rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_init_account_missing_from_post_state() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .find(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .map(|(index, _)| index)
            .unwrap();

        trace[index].post_state.accounts.remove("Cash");

        trace[index].post_state.accounts.insert(
            "Fake".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("0.00").unwrap(),
            },
        );

        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);

        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4026"),
            "expected InitAccount missing-account rejection, got: {}",
            error
        );
    }

    #[test]
    fn verify_execution_witness_rejects_init_account_modified_account() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = bytecode
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, instruction)| {
                matches!(
                    instruction,
                    crate::bytecode::BytecodeInstruction::InitAccount { .. }
                )
            })
            .nth(1)
            .map(|(index, _)| index)
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("99.00").unwrap();

        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4025"),
            "expected InitAccount modified-account rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_pre_state() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Debit|"))
            .unwrap();

        trace[index]
            .pre_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("99.00").unwrap();

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4016"),
            "expected pre-state witness hash mismatch, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_pay_transition() {
        let source = "currency USD\naccount Customer: asset USD = 100\naccount Merchant: asset USD = 0\ntransaction Sale {\n    pay 40 USD\n    from Customer\n    to Merchant\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Pay|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Customer")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("50.00").unwrap();

        trace[index].post_state_hash = state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash = state_hash(&trace[index + 1].pre_state);
        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4069"),
            "expected an independent transition error, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_transfer_transition() {
        let source = "currency USD\naccount Customer: asset USD = 100\naccount Merchant: asset USD = 0\ntransaction Sale {\n    transfer 40 USD\n    from Customer\n    to Merchant\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Transfer|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Customer")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("50.00").unwrap();

        trace[index].post_state_hash = state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash = state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4069"),
            "expected an independent transfer transition error, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_debit_transition() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Debit|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("50.00").unwrap();

        trace[index].post_state_hash = state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash = state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4079"),
            "expected an independent debit transition error, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_credit_transition() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Credit|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("SalesRevenue")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("50.00").unwrap();

        trace[index].post_state_hash = state_hash(&trace[index].post_state);
        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash = state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4089"),
            "expected an independent credit transition error, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_push_money_state_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount Merchant: asset USD = 0\ntransaction Sale {\n    pay 40 USD\n    from Cash\n    to Merchant\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("PushMoney|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_decimal_str("99.00").unwrap();

        trace[index].post_state_hash = crate::verification::state_hash(&trace[index].post_state);

        trace[index + 1].pre_state = trace[index].post_state.clone();
        trace[index + 1].pre_state_hash =
            crate::verification::state_hash(&trace[index + 1].pre_state);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4036"),
            "expected PushMoney state-change rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_push_money_stack_size() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount Merchant: asset USD = 0\ntransaction Sale {\n    pay 40 USD\n    from Cash\n    to Merchant\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("PushMoney|"))
            .unwrap();

        trace[index].post_stack.pop();

        trace[index].post_stack_hash = crate::verification::stack_hash(&trace[index].post_stack);

        trace[index + 1].pre_stack = trace[index].post_stack.clone();
        trace[index + 1].pre_stack_hash =
            crate::verification::stack_hash(&trace[index + 1].pre_stack);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4037"),
            "expected PushMoney stack-size rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_push_money_existing_stack_change() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount Merchant: asset USD = 0\ntransaction Sale {\n    pay 20 USD + 40 USD\n    from Cash\n    to Merchant\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let indices: Vec<usize> = trace
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.instruction.starts_with("PushMoney|"))
            .map(|(index, _)| index)
            .collect();

        assert_eq!(indices.len(), 2);

        let index = indices[1];

        trace[index].post_stack[0].0 = MoneyAmount::from_decimal_str("99.00").unwrap();

        trace[index].post_stack_hash = crate::verification::stack_hash(&trace[index].post_stack);

        trace[index + 1].pre_stack = trace[index].post_stack.clone();
        trace[index + 1].pre_stack_hash =
            crate::verification::stack_hash(&trace[index + 1].pre_stack);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4038"),
            "expected PushMoney existing-stack rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_push_money_incorrect_value() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount Merchant: asset USD = 0\ntransaction Sale {\n    pay 40 USD\n    from Cash\n    to Merchant\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();

        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("PushMoney|"))
            .unwrap();

        trace[index].post_stack[0].0 = MoneyAmount::from_decimal_str("41.00").unwrap();

        trace[index].post_stack_hash = crate::verification::stack_hash(&trace[index].post_stack);

        trace[index + 1].pre_stack = trace[index].post_stack.clone();
        trace[index + 1].pre_stack_hash =
            crate::verification::stack_hash(&trace[index + 1].pre_stack);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4040"),
            "expected PushMoney incorrect-value rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_pay_outside_transaction() {
        let bytecode = crate::bytecode::BytecodeProgram {
            instructions: vec![
                crate::bytecode::BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("40.00").unwrap(),
                    currency: "USD".to_string(),
                },
                crate::bytecode::BytecodeInstruction::Pay {
                    from: "Cash".to_string(),
                    to: "Merchant".to_string(),
                },
            ],
        };

        let mut state = crate::runtime::ExecutionState::new();

        state.accounts.insert(
            "Cash".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("100.00").unwrap(),
            },
        );

        state.accounts.insert(
            "Merchant".to_string(),
            crate::runtime::AccountState {
                account_type: crate::ast::AccountType::Asset,
                currency: "USD".to_string(),
                balance: MoneyAmount::from_decimal_str("0.00").unwrap(),
            },
        );

        let stack = vec![(
            MoneyAmount::from_decimal_str("40.00").unwrap(),
            "USD".to_string(),
        )];

        let trace = vec![
            crate::verification::ExecutionTraceEntry {
                step: 1,
                instruction: bytecode.instructions[0].canonical_representation(),
                pre_state_hash: crate::verification::state_hash(&state),
                post_state_hash: crate::verification::state_hash(&state),
                pre_stack_hash: crate::verification::stack_hash(&[]),
                post_stack_hash: crate::verification::stack_hash(&stack),
                pre_state: state.clone(),
                post_state: state.clone(),
                pre_stack: vec![],
                post_stack: stack.clone(),
            },
            crate::verification::ExecutionTraceEntry {
                step: 2,
                instruction: bytecode.instructions[1].canonical_representation(),
                pre_state_hash: crate::verification::state_hash(&state),
                post_state_hash: crate::verification::state_hash(&state),
                pre_stack_hash: crate::verification::stack_hash(&stack),
                post_stack_hash: crate::verification::stack_hash(&[]),
                pre_state: state.clone(),
                post_state: state,
                pre_stack: stack,
                post_stack: vec![],
            },
        ];

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4057"),
            "expected Pay outside-transaction rejection, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_pre_stack() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("PushMoney|"))
            .unwrap();

        trace[index].pre_stack = vec![(
            MoneyAmount::from_decimal_str("99.00").unwrap(),
            "USD".to_string(),
        )];

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert!(
            error.starts_with("FP4018"),
            "expected pre-stack witness hash mismatch, got: {}",
            error
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_post_stack() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("PushMoney|"))
            .unwrap();

        trace[index].post_stack.push((
            MoneyAmount::from_decimal_str("1.00").unwrap(),
            "USD".to_string(),
        ));

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert_eq!(
            error,
            "FP4019 VERIFICATION_FAILED: post-stack witness hash mismatch at instruction 3."
        );
    }
    #[test]
    fn verify_execution_witness_rejects_tampered_post_state() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        let mut trace = result.trace.clone();
        let index = trace
            .iter()
            .position(|entry| entry.instruction.starts_with("Debit|"))
            .unwrap();

        trace[index]
            .post_state
            .accounts
            .get_mut("Cash")
            .unwrap()
            .balance = MoneyAmount::from_minor_units(5900);

        let error = verify_execution_witness(&bytecode, &trace).unwrap_err();

        assert_eq!(
            error,
            "FP4017 VERIFICATION_FAILED: post-state witness hash mismatch at instruction 4."
        );
    }
    #[test]
    fn verify_execution_witness_accepts_valid_execution() {
        let source = "currency USD\naccount Cash: asset USD = 100\naccount SalesRevenue: revenue USD = 0\ntransaction Sale {\n    debit Cash 40 USD\n    credit SalesRevenue 40 USD\n}";

        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        let program = parser.parse().unwrap();

        crate::types::check_program(&program).unwrap();
        let bytecode = crate::compiler::compile_program(&program).unwrap();
        let result = crate::vm::execute_bytecode(&bytecode).unwrap();

        assert!(verify_execution_witness(&bytecode, &result.trace).is_ok());
    }
}
