use crate::ast::{BinaryOperator, Expression};
use crate::bytecode::BytecodeInstruction;
use crate::types::MoneyAmount;

#[allow(dead_code)]
pub fn compile_expression(expression: &Expression) -> Result<Vec<BytecodeInstruction>, String> {
    let mut instructions = Vec::new();

    compile_expression_into(expression, &mut instructions)?;

    Ok(instructions)
}

fn compile_expression_into(
    expression: &Expression,
    instructions: &mut Vec<BytecodeInstruction>,
) -> Result<(), String> {
    match expression {
        Expression::Money(money) => {
            let amount = MoneyAmount::from_decimal_str(&money.amount)
                .map_err(|error| format!("FP3001 COMPILE_ERROR: {}.", error))?;

            instructions.push(BytecodeInstruction::PushMoney {
                amount,
                currency: money.currency.clone(),
            });
        }

        Expression::Binary {
            left,
            operator,
            right,
        } => {
            compile_expression_into(left, instructions)?;
            compile_expression_into(right, instructions)?;

            match operator {
                BinaryOperator::Add => {
                    instructions.push(BytecodeInstruction::Add);
                }
                BinaryOperator::Subtract => {
                    instructions.push(BytecodeInstruction::Subtract);
                }
            }
        }
    }

    Ok(())
}

pub fn compile_statement(
    statement: &crate::ast::Statement,
) -> Result<Vec<BytecodeInstruction>, String> {
    let mut instructions = Vec::new();

    match statement {
        crate::ast::Statement::Pay(payment) => {
            compile_expression_into(&payment.amount, &mut instructions)?;
            instructions.push(BytecodeInstruction::Pay {
                from: payment.from.clone(),
                to: payment.to.clone(),
            });
        }

        crate::ast::Statement::Transfer(transfer) => {
            compile_expression_into(&transfer.amount, &mut instructions)?;
            instructions.push(BytecodeInstruction::Transfer {
                from: transfer.from.clone(),
                to: transfer.to.clone(),
            });
        }

        crate::ast::Statement::Debit(debit) => {
            compile_expression_into(&debit.amount, &mut instructions)?;
            instructions.push(BytecodeInstruction::Debit {
                account: debit.account.clone(),
            });
        }

        crate::ast::Statement::Credit(credit) => {
            compile_expression_into(&credit.amount, &mut instructions)?;
            instructions.push(BytecodeInstruction::Credit {
                account: credit.account.clone(),
            });
        }
    }

    Ok(instructions)
}
pub fn compile_transaction(
    transaction: &crate::ast::Transaction,
) -> Result<Vec<BytecodeInstruction>, String> {
    let mut instructions = Vec::new();

    for statement in &transaction.statements {
        instructions.extend(compile_statement(statement)?);
    }

    Ok(instructions)
}
pub fn compile_program(
    program: &crate::ast::Program,
) -> Result<crate::bytecode::BytecodeProgram, String> {
    let mut instructions = Vec::new();
    let mut transaction_sequence = 0u64;

    for declaration in &program.declarations {
        match declaration {
            crate::ast::Declaration::Account(account) => {
                instructions.push(BytecodeInstruction::InitAccount {
                    name: account.name.clone(),
                    account_type: account.account_type.clone(),
                    currency: account.currency.clone(),
                    initial_balance: account.initial_balance,
                });
            }

            crate::ast::Declaration::Transaction(transaction) => {
                transaction_sequence += 1;

                instructions.push(BytecodeInstruction::BeginTransaction {
                    name: transaction.name.clone(),
                    sequence: transaction_sequence,
                });

                instructions.extend(compile_transaction(transaction)?);

                instructions.push(BytecodeInstruction::EndTransaction);
            }

            crate::ast::Declaration::Currency(_) => {}
        }
    }

    Ok(crate::bytecode::BytecodeProgram::new(instructions))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::MoneyLiteral;

    #[test]
    fn compiles_pay_statement() {
        let statement = crate::ast::Statement::Pay(crate::ast::Payment {
            amount: Expression::Money(MoneyLiteral {
                amount: "100".to_string(),
                currency: "USD".to_string(),
            }),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
        });

        let bytecode = compile_statement(&statement).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("100").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Pay {
                    from: "Customer".to_string(),
                    to: "Merchant".to_string(),
                },
            ]
        );
    }

    #[test]
    fn compiles_transfer_statement() {
        let statement = crate::ast::Statement::Transfer(crate::ast::Transfer {
            amount: Expression::Money(MoneyLiteral {
                amount: "50".to_string(),
                currency: "USD".to_string(),
            }),
            from: "Customer".to_string(),
            to: "Merchant".to_string(),
        });

        let bytecode = compile_statement(&statement).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("50").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Transfer {
                    from: "Customer".to_string(),
                    to: "Merchant".to_string(),
                },
            ]
        );
    }

    #[test]
    fn compiles_debit_statement() {
        let statement = crate::ast::Statement::Debit(crate::ast::Debit {
            amount: Expression::Money(MoneyLiteral {
                amount: "100".to_string(),
                currency: "USD".to_string(),
            }),
            account: "Cash".to_string(),
        });

        let bytecode = compile_statement(&statement).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("100").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Debit {
                    account: "Cash".to_string(),
                },
            ]
        );
    }

    #[test]
    fn compiles_credit_statement() {
        let statement = crate::ast::Statement::Credit(crate::ast::Credit {
            amount: Expression::Money(MoneyLiteral {
                amount: "100".to_string(),
                currency: "USD".to_string(),
            }),
            account: "Revenue".to_string(),
        });

        let bytecode = compile_statement(&statement).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("100").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Credit {
                    account: "Revenue".to_string(),
                },
            ]
        );
    }
    #[test]
    fn compiles_transaction_in_statement_order() {
        let transaction = crate::ast::Transaction {
            name: "Sale".to_string(),
            statements: vec![
                crate::ast::Statement::Pay(crate::ast::Payment {
                    amount: Expression::Money(MoneyLiteral {
                        amount: "100".to_string(),
                        currency: "USD".to_string(),
                    }),
                    from: "Customer".to_string(),
                    to: "Merchant".to_string(),
                }),
                crate::ast::Statement::Transfer(crate::ast::Transfer {
                    amount: Expression::Money(MoneyLiteral {
                        amount: "25".to_string(),
                        currency: "USD".to_string(),
                    }),
                    from: "Merchant".to_string(),
                    to: "Customer".to_string(),
                }),
            ],
        };

        let bytecode = compile_transaction(&transaction).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("100").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Pay {
                    from: "Customer".to_string(),
                    to: "Merchant".to_string(),
                },
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("25").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Transfer {
                    from: "Merchant".to_string(),
                    to: "Customer".to_string(),
                },
            ]
        );
    }

    #[test]
    fn compiles_program_transactions() {
        let program = crate::ast::Program {
            declarations: vec![crate::ast::Declaration::Transaction(
                crate::ast::Transaction {
                    name: "Sale".to_string(),
                    statements: vec![crate::ast::Statement::Pay(crate::ast::Payment {
                        amount: crate::ast::Expression::Money(crate::ast::MoneyLiteral {
                            amount: "100".to_string(),
                            currency: "USD".to_string(),
                        }),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                },
            )],
        };

        let bytecode = compile_program(&program).unwrap();
        assert!(matches!(
            bytecode.instructions[0],
            BytecodeInstruction::BeginTransaction { .. }
        ));
        assert!(matches!(
            bytecode.instructions[1],
            BytecodeInstruction::PushMoney { .. }
        ));
        assert!(matches!(
            bytecode.instructions[2],
            BytecodeInstruction::Pay { .. }
        ));
        assert!(matches!(
            bytecode.instructions[3],
            BytecodeInstruction::EndTransaction
        ));
    }
    #[test]
    fn compiles_money_literal() {
        let expression = Expression::Money(MoneyLiteral {
            amount: "100".to_string(),
            currency: "USD".to_string(),
        });

        let bytecode = compile_expression(&expression).unwrap();

        assert_eq!(
            bytecode,
            vec![BytecodeInstruction::PushMoney {
                amount: MoneyAmount::from_decimal_str("100").unwrap(),
                currency: "USD".to_string(),
            }]
        );
    }

    #[test]
    fn compiles_addition() {
        let expression = Expression::Binary {
            left: Box::new(Expression::Money(MoneyLiteral {
                amount: "100".to_string(),
                currency: "USD".to_string(),
            })),
            operator: BinaryOperator::Add,
            right: Box::new(Expression::Money(MoneyLiteral {
                amount: "50".to_string(),
                currency: "USD".to_string(),
            })),
        };

        let bytecode = compile_expression(&expression).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("100").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("50").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Add,
            ]
        );
    }

    #[test]
    fn compiles_nested_expression_in_postfix_order() {
        let expression = Expression::Binary {
            left: Box::new(Expression::Binary {
                left: Box::new(Expression::Money(MoneyLiteral {
                    amount: "100".to_string(),
                    currency: "USD".to_string(),
                })),
                operator: BinaryOperator::Add,
                right: Box::new(Expression::Money(MoneyLiteral {
                    amount: "50".to_string(),
                    currency: "USD".to_string(),
                })),
            }),
            operator: BinaryOperator::Subtract,
            right: Box::new(Expression::Money(MoneyLiteral {
                amount: "25".to_string(),
                currency: "USD".to_string(),
            })),
        };

        let bytecode = compile_expression(&expression).unwrap();

        assert_eq!(
            bytecode,
            vec![
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("100").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("50").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Add,
                BytecodeInstruction::PushMoney {
                    amount: MoneyAmount::from_decimal_str("25").unwrap(),
                    currency: "USD".to_string(),
                },
                BytecodeInstruction::Subtract,
            ]
        );
    }
}
