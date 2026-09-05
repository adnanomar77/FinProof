#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    Currency(CurrencyDeclaration),
    Account(AccountDeclaration),
    Transaction(Transaction),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrencyDeclaration {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountDeclaration {
    pub name: String,
    pub account_type: AccountType,
    pub currency: String,
    pub initial_balance: crate::types::MoneyAmount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub name: String,
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Pay(Payment),
    Transfer(Transfer),
    Debit(Debit),
    Credit(Credit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Payment {
    pub amount: Expression,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transfer {
    pub amount: Expression,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Debit {
    pub account: String,
    pub amount: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Credit {
    pub account: String,
    pub amount: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Money(MoneyLiteral),

    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MoneyLiteral {
    pub amount: String,
    pub currency: String,
}

#[allow(dead_code)]
impl Program {
    pub fn canonical_representation(&self) -> String {
        self.declarations
            .iter()
            .map(Declaration::canonical_representation)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Declaration {
    fn canonical_representation(&self) -> String {
        match self {
            Self::Currency(currency) => {
                format!("Currency|code={}", currency.code)
            }

            Self::Account(account) => {
                let account_type = match account.account_type {
                    AccountType::Asset => "Asset",
                    AccountType::Liability => "Liability",
                    AccountType::Equity => "Equity",
                    AccountType::Revenue => "Revenue",
                    AccountType::Expense => "Expense",
                };

                format!(
                    "Account|name={}|account_type={}|currency={}|initial_balance_minor_units={}",
                    account.name,
                    account_type,
                    account.currency,
                    account.initial_balance.minor_units()
                )
            }

            Self::Transaction(transaction) => {
                let statements = transaction
                    .statements
                    .iter()
                    .map(Statement::canonical_representation)
                    .collect::<Vec<_>>()
                    .join(";");

                format!(
                    "Transaction|name={}|statements={}",
                    transaction.name, statements
                )
            }
        }
    }
}

impl Statement {
    fn canonical_representation(&self) -> String {
        match self {
            Self::Pay(payment) => format!(
                "Pay|from={}|to={}|amount={}",
                payment.from,
                payment.to,
                payment.amount.canonical_representation()
            ),

            Self::Transfer(transfer) => format!(
                "Transfer|from={}|to={}|amount={}",
                transfer.from,
                transfer.to,
                transfer.amount.canonical_representation()
            ),

            Self::Debit(debit) => format!(
                "Debit|account={}|amount={}",
                debit.account,
                debit.amount.canonical_representation()
            ),

            Self::Credit(credit) => format!(
                "Credit|account={}|amount={}",
                credit.account,
                credit.amount.canonical_representation()
            ),
        }
    }
}

impl Expression {
    fn canonical_representation(&self) -> String {
        match self {
            Self::Money(money) => {
                let amount = crate::types::MoneyAmount::from_decimal_str(&money.amount)
                    .expect("type checker guarantees valid monetary literals");

                format!(
                    "Money|amount_minor_units={}|currency={}",
                    amount.minor_units(),
                    money.currency
                )
            }

            Self::Binary {
                left,
                operator,
                right,
            } => {
                let operator = match operator {
                    BinaryOperator::Add => "Add",
                    BinaryOperator::Subtract => "Subtract",
                };

                format!(
                    "Binary|operator={}|left=[{}]|right=[{}]",
                    operator,
                    left.canonical_representation(),
                    right.canonical_representation()
                )
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_currency() {
        let currency = CurrencyDeclaration {
            code: "USD".to_string(),
        };

        assert_eq!(currency.code, "USD");
    }

    #[test]
    fn creates_asset_account() {
        let account = AccountDeclaration {
            name: "Cash".to_string(),
            account_type: AccountType::Asset,
            currency: "USD".to_string(),
            initial_balance: crate::types::MoneyAmount::from_minor_units(0),
        };

        assert_eq!(account.name, "Cash");
        assert_eq!(account.account_type, AccountType::Asset);
        assert_eq!(account.currency, "USD");
    }

    #[test]
    fn creates_liability_account() {
        let account = AccountDeclaration {
            name: "Loan".to_string(),
            account_type: AccountType::Liability,
            currency: "USD".to_string(),
            initial_balance: crate::types::MoneyAmount::from_minor_units(0),
        };

        assert_eq!(account.account_type, AccountType::Liability);
    }

    #[test]
    fn account_types_are_distinct() {
        assert_ne!(AccountType::Asset, AccountType::Liability);
        assert_ne!(AccountType::Revenue, AccountType::Expense);
    }

    #[test]
    fn creates_transfer() {
        let transfer = Transfer {
            amount: Expression::Money(MoneyLiteral {
                amount: "250".to_string(),
                currency: "USD".to_string(),
            }),
            from: "Cash".to_string(),
            to: "Savings".to_string(),
        };

        assert_eq!(transfer.from, "Cash");
        assert_eq!(transfer.to, "Savings");
    }

    #[test]
    fn statement_supports_transfer() {
        let statement = Statement::Transfer(Transfer {
            amount: Expression::Money(MoneyLiteral {
                amount: "250".to_string(),
                currency: "USD".to_string(),
            }),
            from: "Cash".to_string(),
            to: "Savings".to_string(),
        });

        assert!(matches!(statement, Statement::Transfer(_)));
    }
}
