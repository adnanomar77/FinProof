use crate::types::MoneyAmount;

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeInstruction {
    InitAccount {
        name: String,
        account_type: crate::ast::AccountType,
        currency: String,
        initial_balance: MoneyAmount,
    },
    BeginTransaction {
        name: String,
        sequence: u64,
    },
    EndTransaction,
    PushMoney {
        amount: MoneyAmount,
        currency: String,
    },
    Add,
    Subtract,
    Pay {
        from: String,
        to: String,
    },
    Transfer {
        from: String,
        to: String,
    },
    Debit {
        account: String,
    },
    Credit {
        account: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeProgram {
    pub instructions: Vec<BytecodeInstruction>,
}

impl BytecodeInstruction {
    pub fn canonical_representation(&self) -> String {
        match self {
            Self::InitAccount {
                name,
                account_type,
                currency,
                initial_balance,
            } => {
                let account_type = match account_type {
                    crate::ast::AccountType::Asset => "Asset",
                    crate::ast::AccountType::Liability => "Liability",
                    crate::ast::AccountType::Equity => "Equity",
                    crate::ast::AccountType::Revenue => "Revenue",
                    crate::ast::AccountType::Expense => "Expense",
                };

                format!(
                    "InitAccount|name={}|account_type={}|currency={}|initial_balance_minor_units={}",
                    name,
                    account_type,
                    currency,
                    initial_balance.minor_units()
                )
            }

            Self::BeginTransaction { name, sequence } => {
                format!("BeginTransaction|name={}|sequence={}", name, sequence)
            }

            Self::EndTransaction => "EndTransaction".to_string(),

            Self::PushMoney { amount, currency } => {
                format!(
                    "PushMoney|amount_minor_units={}|currency={}",
                    amount.minor_units(),
                    currency
                )
            }

            Self::Add => "Add".to_string(),

            Self::Subtract => "Subtract".to_string(),

            Self::Pay { from, to } => {
                format!("Pay|from={}|to={}", from, to)
            }

            Self::Transfer { from, to } => {
                format!("Transfer|from={}|to={}", from, to)
            }

            Self::Debit { account } => {
                format!("Debit|account={}", account)
            }

            Self::Credit { account } => {
                format!("Credit|account={}", account)
            }
        }
    }
}
impl BytecodeProgram {
    pub fn new(instructions: Vec<BytecodeInstruction>) -> Self {
        Self { instructions }
    }

    pub fn canonical_representation(&self) -> String {
        self.instructions
            .iter()
            .map(BytecodeInstruction::canonical_representation)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_empty_bytecode_program() {
        let program = BytecodeProgram::new(Vec::new());

        assert!(program.instructions.is_empty());
    }

    #[test]
    fn creates_push_money_instruction() {
        let instruction = BytecodeInstruction::PushMoney {
            amount: MoneyAmount::from_decimal_str("100").unwrap(),
            currency: "USD".to_string(),
        };

        assert!(matches!(instruction, BytecodeInstruction::PushMoney { .. }));
    }

    #[test]
    fn creates_arithmetic_instructions() {
        assert_eq!(BytecodeInstruction::Add, BytecodeInstruction::Add);
        assert_eq!(BytecodeInstruction::Subtract, BytecodeInstruction::Subtract);
    }

    #[test]
    fn creates_pay_instruction() {
        let instruction = BytecodeInstruction::Pay {
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
        };

        assert!(matches!(instruction, BytecodeInstruction::Pay { .. }));
    }

    #[test]
    fn creates_transfer_instruction() {
        let instruction = BytecodeInstruction::Transfer {
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
        };

        assert!(matches!(instruction, BytecodeInstruction::Transfer { .. }));
    }

    #[test]
    fn creates_debit_and_credit_instructions() {
        let debit = BytecodeInstruction::Debit {
            account: "Cash".to_string(),
        };

        let credit = BytecodeInstruction::Credit {
            account: "Revenue".to_string(),
        };

        assert!(matches!(debit, BytecodeInstruction::Debit { .. }));
        assert!(matches!(credit, BytecodeInstruction::Credit { .. }));
    }

    #[test]
    fn bytecode_program_canonical_representation_is_deterministic() {
        let program = BytecodeProgram::new(vec![
            BytecodeInstruction::BeginTransaction {
                name: "Sale".to_string(),
                sequence: 1,
            },
            BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100.00").unwrap(),
                currency: "USD".to_string(),
            },
            BytecodeInstruction::Pay {
                from: "Customer".to_string(),
                to: "Merchant".to_string(),
            },
            BytecodeInstruction::EndTransaction,
        ]);

        let first = program.canonical_representation();
        let second = program.canonical_representation();

        assert_eq!(first, second);
        assert_eq!(
            first,
            "BeginTransaction|name=Sale|sequence=1\nPushMoney|amount_minor_units=10000|currency=USD\nPay|from=Customer|to=Merchant\nEndTransaction"
        );
    }
}
