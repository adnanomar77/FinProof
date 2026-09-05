use crate::ast::{AccountType as AstAccountType, BinaryOperator, Declaration, Expression, Program};
use std::collections::HashMap;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinancialType {
    Money { currency: String },
    Account(AccountType),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}
impl FinancialType {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoneyAmount {
    minor_units: i64,
}
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Sub};
impl PartialOrd for MoneyAmount {
    fn partial_cmp(&self, other: &MoneyAmount) -> Option<Ordering> {
        self.minor_units.partial_cmp(&other.minor_units)
    }
}
impl Add for MoneyAmount {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::from_minor_units(self.minor_units + other.minor_units)
    }
}
impl Sub for MoneyAmount {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::from_minor_units(self.minor_units - other.minor_units)
    }
}
impl fmt::Display for MoneyAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.minor_units < 0;
        let absolute = if negative {
            -(self.minor_units as i128)
        } else {
            self.minor_units as i128
        };
        let whole = absolute / 100;
        let fraction = absolute % 100;
        if negative {
            write!(f, "-{}.{:02}", whole, fraction)
        } else {
            write!(f, "{}.{:02}", whole, fraction)
        }
    }
}
impl MoneyAmount {
    pub fn from_minor_units(minor_units: i64) -> Self {
        Self { minor_units }
    }
    pub fn from_decimal_str(value: &str) -> Result<Self, String> {
        let negative = value.starts_with('-');
        let value = value.strip_prefix('-').unwrap_or(value);
        let parts: Vec<&str> = value.split('.').collect();
        if parts.len() > 2 || parts.iter().any(|part| part.is_empty()) {
            return Err(format!("invalid monetary value '{}'", value));
        }
        let whole: i64 = parts[0]
            .parse()
            .map_err(|_| format!("invalid monetary value '{}'", value))?;
        let fraction = if parts.len() == 2 {
            if parts[1].len() > 2 {
                return Err(format!(
                    "monetary value '{}' has more than 2 decimal places",
                    value
                ));
            }
            format!("{:0<2}", parts[1])
                .parse::<i64>()
                .map_err(|_| format!("invalid monetary value '{}'", value))?
        } else {
            0
        };
        let minor_units = whole
            .checked_mul(100)
            .and_then(|v| v.checked_add(fraction))
            .ok_or_else(|| format!("monetary value '{}' is too large", value))?;
        Ok(Self {
            minor_units: if negative { -minor_units } else { minor_units },
        })
    }
    pub fn minor_units(&self) -> i64 {
        self.minor_units
    }
    pub fn checked_add(self, other: Self) -> Result<Self, String> {
        self.minor_units
            .checked_add(other.minor_units)
            .map(Self::from_minor_units)
            .ok_or_else(|| "monetary arithmetic overflow".to_string())
    }
    pub fn checked_sub(self, other: Self) -> Result<Self, String> {
        self.minor_units
            .checked_sub(other.minor_units)
            .map(Self::from_minor_units)
            .ok_or_else(|| "monetary arithmetic overflow".to_string())
    }
}
pub fn infer_expression_type(expression: &Expression) -> Result<FinancialType, String> {
    match expression {
        Expression::Money(money) => Ok(FinancialType::Money {
            currency: money.currency.clone(),
        }),
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let left_type = infer_expression_type(left)?;
            let right_type = infer_expression_type(right)?;
            check_same_currency(&left_type, &right_type)?;
            match operator {
                BinaryOperator::Add | BinaryOperator::Subtract => {
                    match left_type {
                        FinancialType::Money { currency } => Ok(FinancialType::Money { currency }),
                        _ => Err("FP1002 TYPE_MISMATCH: arithmetic requires monetary values."
                            .to_string()),
                    }
                }
            }
        }
    }
}
pub fn check_same_currency(left: &FinancialType, right: &FinancialType) -> Result<(), String> {
    match (left, right) {
        (
            FinancialType::Money {
                currency: left_currency,
            },
            FinancialType::Money {
                currency: right_currency,
            },
        ) => {
            if left_currency != right_currency {
                return Err(format!(
                    "FP1003 CURRENCY_MISMATCH: cannot combine {} with {}.",
                    left_currency, right_currency
                ));
            }
            Ok(())
        }
        _ => Err("FP1002 TYPE_MISMATCH: incompatible financial types.".to_string()),
    }
}
fn account_type_to_financial_type(account_type: &AstAccountType) -> FinancialType {
    let account_type = match account_type {
        AstAccountType::Asset => AccountType::Asset,
        AstAccountType::Liability => AccountType::Liability,
        AstAccountType::Equity => AccountType::Equity,
        AstAccountType::Revenue => AccountType::Revenue,
        AstAccountType::Expense => AccountType::Expense,
    };
    FinancialType::Account(account_type)
}
fn validate_expression_currencies(
    expression: &Expression,
    expected_currency: &str,
) -> Result<(), String> {
    match expression {
        Expression::Money(money) => {
            if money.currency != expected_currency {
                return Err(format!(
                    "FP1003 CURRENCY_MISMATCH: expected {}, found {}.",
                    expected_currency, money.currency
                ));
            }
            Ok(())
        }
        Expression::Binary { left, right, .. } => {
            validate_expression_currencies(left, expected_currency)?;
            validate_expression_currencies(right, expected_currency)
        }
    }
}
pub fn check_program(program: &Program) -> Result<(), String> {
    let mut currencies: HashMap<String, bool> = HashMap::new();
    let mut accounts: HashMap<String, (FinancialType, String)> = HashMap::new();
    for declaration in &program.declarations {
        match declaration {
            Declaration::Currency(currency) => {
                if currencies.insert(currency.code.clone(), true).is_some() {
                    return Err(format!(
                        "FP1004 DUPLICATE_CURRENCY: currency '{}' is already defined.",
                        currency.code
                    ));
                }
            }
            Declaration::Account(account) => {
                if accounts.contains_key(&account.name) {
                    return Err(format!(
                        "FP1005 DUPLICATE_ACCOUNT: account '{}' is already defined.",
                        account.name
                    ));
                }
                if !currencies.contains_key(&account.currency) {
                    return Err(format!(
                        "FP1006 UNKNOWN_CURRENCY: currency '{}' is not defined.",
                        account.currency
                    ));
                }
                if account.initial_balance.minor_units() < 0 {
                    return Err(format!(
                        "FP1007 INVALID_INITIAL_BALANCE: account '{}' cannot have a negative initial balance.",
                        account.name
                    ));
                }
                accounts.insert(
                    account.name.clone(),
                    (
                        account_type_to_financial_type(&account.account_type),
                        account.currency.clone(),
                    ),
                );
            }
            Declaration::Transaction(transaction) => {
                let mut debit_totals: HashMap<String, MoneyAmount> = HashMap::new();
                let mut credit_totals: HashMap<String, MoneyAmount> = HashMap::new();
                for statement in &transaction.statements {
                    match statement {
                        crate::ast::Statement::Pay(payment) => {
                            validate_payment(payment, &accounts, &currencies)?;
                        }
                        crate::ast::Statement::Transfer(transfer) => {
                            validate_transfer(transfer, &accounts, &currencies)?;
                        }
                        crate::ast::Statement::Debit(debit) => {
                            validate_debit(debit, &accounts, &currencies, &mut debit_totals)?;
                        }
                        crate::ast::Statement::Credit(credit) => {
                            validate_credit(credit, &accounts, &currencies, &mut credit_totals)?;
                        }
                    }
                }
                validate_double_entry_balance(&transaction.name, &debit_totals, &credit_totals)?;
            }
        }
    }
    Ok(())
}
fn validate_payment(
    payment: &crate::ast::Payment,
    accounts: &HashMap<String, (FinancialType, String)>,
    currencies: &HashMap<String, bool>,
) -> Result<(), String> {
    validate_move_expression(
        &payment.amount,
        &payment.from,
        &payment.to,
        accounts,
        currencies,
        "payment",
    )
}
fn validate_transfer(
    transfer: &crate::ast::Transfer,
    accounts: &HashMap<String, (FinancialType, String)>,
    currencies: &HashMap<String, bool>,
) -> Result<(), String> {
    validate_move_expression(
        &transfer.amount,
        &transfer.from,
        &transfer.to,
        accounts,
        currencies,
        "transfer",
    )
}
fn validate_move_expression(
    expression: &Expression,
    from: &str,
    to: &str,
    accounts: &HashMap<String, (FinancialType, String)>,
    currencies: &HashMap<String, bool>,
    operation: &str,
) -> Result<(), String> {
    let expression_type = infer_expression_type(expression)?;
    let currency = match expression_type {
        FinancialType::Money { currency } => currency,
        _ => return Err("FP1002 TYPE_MISMATCH: operation requires monetary value.".to_string()),
    };
    if !currencies.contains_key(&currency) {
        return Err(format!(
            "FP1006 UNKNOWN_CURRENCY: currency '{}' is not defined.",
            currency
        ));
    }
    if from == to {
        return Err(format!(
            "FP1008 SELF_TRANSFER: account '{}' cannot transfer to itself.",
            from
        ));
    }
    let from_account = accounts
        .get(from)
        .ok_or_else(|| format!("FP1001 UNKNOWN_ACCOUNT: account '{}' is not defined.", from))?;
    let to_account = accounts
        .get(to)
        .ok_or_else(|| format!("FP1001 UNKNOWN_ACCOUNT: account '{}' is not defined.", to))?;
    if from_account.1 != currency || to_account.1 != currency {
        return Err(format!(
            "FP1003 CURRENCY_MISMATCH: {} requires currency {}.",
            operation, currency
        ));
    }
    validate_expression_currencies(expression, &currency)?;
    Ok(())
}
fn validate_debit(
    debit: &crate::ast::Debit,
    accounts: &HashMap<String, (FinancialType, String)>,
    currencies: &HashMap<String, bool>,
    debit_totals: &mut HashMap<String, MoneyAmount>,
) -> Result<(), String> {
    validate_entry(
        &debit.account,
        &debit.amount,
        accounts,
        currencies,
        debit_totals,
        "debit",
    )
}
fn validate_credit(
    credit: &crate::ast::Credit,
    accounts: &HashMap<String, (FinancialType, String)>,
    currencies: &HashMap<String, bool>,
    credit_totals: &mut HashMap<String, MoneyAmount>,
) -> Result<(), String> {
    validate_entry(
        &credit.account,
        &credit.amount,
        accounts,
        currencies,
        credit_totals,
        "credit",
    )
}
fn validate_entry(
    account_name: &str,
    expression: &Expression,
    accounts: &HashMap<String, (FinancialType, String)>,
    currencies: &HashMap<String, bool>,
    totals: &mut HashMap<String, MoneyAmount>,
    operation: &str,
) -> Result<(), String> {
    let (account_type, account_currency) = accounts.get(account_name).ok_or_else(|| {
        format!(
            "FP1001 UNKNOWN_ACCOUNT: account '{}' is not defined.",
            account_name
        )
    })?;
    let account_type = match account_type {
        FinancialType::Account(account_type) => account_type,
        _ => {
            return Err(format!(
                "FP1002 TYPE_MISMATCH: {} requires an account.",
                operation
            ));
        }
    };
    let operation_allowed = matches!((operation, account_type), ("debit", AccountType::Asset | AccountType::Expense) | ("credit", AccountType::Liability | AccountType::Equity | AccountType::Revenue));
    if !operation_allowed {
        return Err(format!(
            "FP1017 INVALID_ACCOUNT_OPERATION: {} is not valid for {:?} account '{}'.",
            operation, account_type, account_name
        ));
    }
    if !currencies.contains_key(account_currency) {
        return Err(format!(
            "FP1006 UNKNOWN_CURRENCY: currency '{}' is not defined.",
            account_currency
        ));
    }
    let expression_type = infer_expression_type(expression)?;
    let currency = match expression_type {
        FinancialType::Money { currency } => currency,
        _ => {
            return Err(format!(
                "FP1002 TYPE_MISMATCH: {} requires a monetary value.",
                operation
            ));
        }
    };
    if currency != *account_currency {
        return Err(format!(
            "FP1003 CURRENCY_MISMATCH: expression uses {}, but account uses {}.",
            currency, account_currency
        ));
    }
    validate_expression_currencies(expression, account_currency)?;
    let amount = expression_amount(expression)?;
    if amount.minor_units() <= 0 {
        return Err(format!(
            "FP1009 INVALID_AMOUNT: {} amount must be greater than zero.",
            operation
        ));
    }
    let current = totals
        .get(account_currency)
        .copied()
        .unwrap_or_else(|| MoneyAmount::from_minor_units(0));
    let total = current
        .checked_add(amount)
        .map_err(|_| "FP2009 ARITHMETIC_OVERFLOW: double-entry total overflow.".to_string())?;
    totals.insert(account_currency.clone(), total);
    Ok(())
}
fn expression_amount(expression: &Expression) -> Result<MoneyAmount, String> {
    match expression {
        Expression::Money(money) => MoneyAmount::from_decimal_str(&money.amount)
            .map_err(|e| format!("FP1009 INVALID_AMOUNT: {}", e)),
        Expression::Binary {
            left,
            operator,
            right,
        } => {
            let left_amount = expression_amount(left)?;
            let right_amount = expression_amount(right)?;
            match operator {
                BinaryOperator::Add => left_amount.checked_add(right_amount).map_err(|_| {
                    "FP2009 ARITHMETIC_OVERFLOW: monetary expression addition overflow.".to_string()
                }),
                BinaryOperator::Subtract => left_amount.checked_sub(right_amount).map_err(|_| {
                    "FP2009 ARITHMETIC_OVERFLOW: monetary expression subtraction overflow."
                        .to_string()
                }),
            }
        }
    }
}
pub fn validate_double_entry_balance(
    transaction_name: &str,
    debit_totals: &HashMap<String, MoneyAmount>,
    credit_totals: &HashMap<String, MoneyAmount>,
) -> Result<(), String> {
    let mut currencies = debit_totals.keys().cloned().collect::<Vec<_>>();
    for currency in credit_totals.keys() {
        if !currencies.contains(currency) {
            currencies.push(currency.clone());
        }
    }
    currencies.sort();
    for currency in currencies {
        let debit = debit_totals
            .get(&currency)
            .copied()
            .unwrap_or_else(|| MoneyAmount::from_minor_units(0));
        let credit = credit_totals
            .get(&currency)
            .copied()
            .unwrap_or_else(|| MoneyAmount::from_minor_units(0));
        if debit != credit {
            return Err(format!(
                "FP1016 UNBALANCED_TRANSACTION: transaction '{}' is unbalanced in {}: debit {} != credit {}.",
                transaction_name, currency, debit, credit
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod money_amount_tests {
    use super::MoneyAmount;
    #[test]
    fn parses_decimal_money_exactly() {
        assert_eq!(
            MoneyAmount::from_decimal_str("0.10").unwrap().minor_units(),
            10
        );
        assert_eq!(
            MoneyAmount::from_decimal_str("12.34")
                .unwrap()
                .minor_units(),
            1234
        );
        assert_eq!(
            MoneyAmount::from_decimal_str("5").unwrap().minor_units(),
            500
        );
    }
    #[test]
    fn rejects_decimal_money_overflow() {
        assert!(MoneyAmount::from_decimal_str("92233720368547758.08").is_err());
    }
    #[test]
    fn supports_addition() {
        let a = MoneyAmount::from_decimal_str("10.25").unwrap();
        let b = MoneyAmount::from_decimal_str("2.75").unwrap();
        assert_eq!((a + b).minor_units(), 1300);
    }
    #[test]
    fn supports_subtraction() {
        let a = MoneyAmount::from_decimal_str("10.25").unwrap();
        let b = MoneyAmount::from_decimal_str("2.75").unwrap();
        assert_eq!((a - b).minor_units(), 750);
    }
    #[test]
    fn rejects_more_than_two_decimal_places() {
        assert!(MoneyAmount::from_decimal_str("1.001").is_err());
    }
    #[test]
    fn parses_integer_money() {
        assert_eq!(
            MoneyAmount::from_decimal_str("100").unwrap(),
            MoneyAmount::from_minor_units(10000)
        );
    }
    #[test]
    fn parses_negative_money() {
        assert_eq!(
            MoneyAmount::from_decimal_str("-1.25")
                .unwrap()
                .minor_units(),
            -125
        );
    }
    #[test]
    fn displays_money_exactly() {
        assert_eq!(
            MoneyAmount::from_decimal_str("12.30").unwrap().to_string(),
            "12.30"
        );
    }
}
#[cfg(test)]
mod type_checker_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType, BinaryOperator, Credit, CurrencyDeclaration, Debit,
        Declaration, Expression, MoneyLiteral, Payment, Program, Statement, Transaction, Transfer,
    };
    fn money(amount: &str, currency: &str) -> Expression {
        Expression::Money(MoneyLiteral {
            amount: amount.to_string(),
            currency: currency.to_string(),
        })
    }
    fn currency(code: &str) -> Declaration {
        Declaration::Currency(CurrencyDeclaration {
            code: code.to_string(),
        })
    }
    fn account(
        name: &str,
        account_type: AccountType,
        currency: &str,
        balance: &str,
    ) -> Declaration {
        Declaration::Account(AccountDeclaration {
            name: name.to_string(),
            account_type,
            currency: currency.to_string(),
            initial_balance: MoneyAmount::from_decimal_str(balance).unwrap(),
        })
    }
    #[test]
    fn infers_money_type() {
        let result = infer_expression_type(&money("10", "USD")).unwrap();
        assert_eq!(
            result,
            FinancialType::Money {
                currency: "USD".to_string()
            }
        );
    }
    #[test]
    fn rejects_mixed_currency_expression() {
        let expression = Expression::Binary {
            left: Box::new(money("10", "USD")),
            operator: BinaryOperator::Add,
            right: Box::new(money("5", "EUR")),
        };
        assert!(infer_expression_type(&expression).is_err());
    }
    #[test]
    fn accepts_same_currency_expression() {
        let expression = Expression::Binary {
            left: Box::new(money("10", "USD")),
            operator: BinaryOperator::Add,
            right: Box::new(money("5", "USD")),
        };
        assert!(infer_expression_type(&expression).is_ok());
    }
    #[test]
    fn accepts_balanced_double_entry() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("SalesRevenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "Cash".to_string(),
                            amount: money("100", "USD"),
                        }),
                        Statement::Credit(Credit {
                            account: "SalesRevenue".to_string(),
                            amount: money("100", "USD"),
                        }),
                    ],
                }),
            ],
        };
        assert!(check_program(&program).is_ok());
    }
    #[test]
    fn rejects_debit_revenue_account() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("SalesRevenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "InvalidDebit".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "SalesRevenue".to_string(),
                            amount: money("100", "USD"),
                        }),
                        Statement::Credit(Credit {
                            account: "Cash".to_string(),
                            amount: money("100", "USD"),
                        }),
                    ],
                }),
            ],
        };
        let error = check_program(&program).unwrap_err();
        assert!(error.contains("FP1017 INVALID_ACCOUNT_OPERATION"));
    }

    #[test]
    fn rejects_credit_asset_account() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "100"),
                account("SalesRevenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "InvalidCredit".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "SalesRevenue".to_string(),
                            amount: money("40", "USD"),
                        }),
                        Statement::Credit(Credit {
                            account: "Cash".to_string(),
                            amount: money("40", "USD"),
                        }),
                    ],
                }),
            ],
        };
        let error = check_program(&program).unwrap_err();
        assert!(error.contains("FP1017 INVALID_ACCOUNT_OPERATION"));
    }

    #[test]
    fn rejects_unbalanced_double_entry() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("SalesRevenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "Cash".to_string(),
                            amount: money("100", "USD"),
                        }),
                        Statement::Credit(Credit {
                            account: "SalesRevenue".to_string(),
                            amount: money("90", "USD"),
                        }),
                    ],
                }),
            ],
        };
        let error = check_program(&program).unwrap_err();
        assert!(error.contains("FP1016 UNBALANCED_TRANSACTION"));
    }
    #[test]
    fn accepts_compound_balanced_double_entry() {
        let debit_expression = Expression::Binary {
            left: Box::new(money("100", "USD")),
            operator: BinaryOperator::Add,
            right: Box::new(money("50", "USD")),
        };
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("SalesRevenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "Cash".to_string(),
                            amount: debit_expression,
                        }),
                        Statement::Credit(Credit {
                            account: "SalesRevenue".to_string(),
                            amount: money("150", "USD"),
                        }),
                    ],
                }),
            ],
        };
        assert!(check_program(&program).is_ok());
    }
    #[test]
    fn rejects_double_entry_currency_mismatch() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                currency("EUR"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("Revenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "Cash".to_string(),
                            amount: money("100", "EUR"),
                        }),
                        Statement::Credit(Credit {
                            account: "Revenue".to_string(),
                            amount: money("100", "EUR"),
                        }),
                    ],
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn rejects_zero_double_entry() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("Revenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "Cash".to_string(),
                            amount: money("0", "USD"),
                        }),
                        Statement::Credit(Credit {
                            account: "Revenue".to_string(),
                            amount: money("0", "USD"),
                        }),
                    ],
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn rejects_negative_double_entry() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Cash", AccountType::Asset, "USD", "0"),
                account("Revenue", AccountType::Revenue, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![
                        Statement::Debit(Debit {
                            account: "Cash".to_string(),
                            amount: money("-10", "USD"),
                        }),
                        Statement::Credit(Credit {
                            account: "Revenue".to_string(),
                            amount: money("-10", "USD"),
                        }),
                    ],
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn accepts_payment() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("Customer", AccountType::Asset, "USD", "100"),
                account("Merchant", AccountType::Asset, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![Statement::Pay(Payment {
                        amount: money("50", "USD"),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        assert!(check_program(&program).is_ok());
    }
    #[test]
    fn accepts_transfer() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("A", AccountType::Asset, "USD", "100"),
                account("B", AccountType::Asset, "USD", "0"),
                Declaration::Transaction(Transaction {
                    name: "Move".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: money("25", "USD"),
                        from: "A".to_string(),
                        to: "B".to_string(),
                    })],
                }),
            ],
        };
        assert!(check_program(&program).is_ok());
    }
    #[test]
    fn rejects_unknown_account() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("A", AccountType::Asset, "USD", "100"),
                Declaration::Transaction(Transaction {
                    name: "Move".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: money("25", "USD"),
                        from: "A".to_string(),
                        to: "Missing".to_string(),
                    })],
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn rejects_self_transfer() {
        let program = Program {
            declarations: vec![
                currency("USD"),
                account("A", AccountType::Asset, "USD", "100"),
                Declaration::Transaction(Transaction {
                    name: "Move".to_string(),
                    statements: vec![Statement::Transfer(Transfer {
                        amount: money("25", "USD"),
                        from: "A".to_string(),
                        to: "A".to_string(),
                    })],
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
}
#[cfg(test)]
mod additional_type_tests {
    use super::*;
    use crate::ast::{
        AccountDeclaration, AccountType as AstAccountType, BinaryOperator, CurrencyDeclaration,
        Declaration, Expression, MoneyLiteral, Payment, Program, Statement, Transaction, Transfer,
    };
    fn money(amount: &str, currency: &str) -> Expression {
        Expression::Money(MoneyLiteral {
            amount: amount.to_string(),
            currency: currency.to_string(),
        })
    }
    fn base_program() -> Vec<Declaration> {
        vec![
            Declaration::Currency(CurrencyDeclaration {
                code: "USD".to_string(),
            }),
            Declaration::Account(AccountDeclaration {
                name: "Cash".to_string(),
                account_type: AstAccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(0),
            }),
            Declaration::Account(AccountDeclaration {
                name: "Revenue".to_string(),
                account_type: AstAccountType::Revenue,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(0),
            }),
        ]
    }
    #[test]
    fn financial_type_money_constructor_works() {
        assert_eq!(
            FinancialType::Money {
                currency: "USD".to_string()
            },
            FinancialType::Money {
                currency: "USD".to_string()
            }
        );
    }
    #[test]
    fn financial_type_account_constructor_works() {
        assert_eq!(
            FinancialType::Account(AccountType::Asset),
            FinancialType::Account(AccountType::Asset)
        );
    }
    #[test]
    fn same_currency_money_values_are_accepted() {
        let left = FinancialType::Money {
            currency: "USD".to_string(),
        };
        let right = FinancialType::Money {
            currency: "USD".to_string(),
        };
        assert!(check_same_currency(&left, &right).is_ok());
    }
    #[test]
    fn different_currency_money_values_are_rejected() {
        let left = FinancialType::Money {
            currency: "USD".to_string(),
        };
        let right = FinancialType::Money {
            currency: "EUR".to_string(),
        };
        assert!(check_same_currency(&left, &right).is_err());
    }
    #[test]
    fn nested_same_currency_expression_is_accepted() {
        let expression = Expression::Binary {
            left: Box::new(Expression::Binary {
                left: Box::new(money("10", "USD")),
                operator: BinaryOperator::Add,
                right: Box::new(money("20", "USD")),
            }),
            operator: BinaryOperator::Add,
            right: Box::new(money("30", "USD")),
        };
        assert!(infer_expression_type(&expression).is_ok());
    }
    #[test]
    fn nested_mixed_currency_expression_is_rejected() {
        let expression = Expression::Binary {
            left: Box::new(Expression::Binary {
                left: Box::new(money("10", "USD")),
                operator: BinaryOperator::Add,
                right: Box::new(money("20", "USD")),
            }),
            operator: BinaryOperator::Add,
            right: Box::new(money("30", "EUR")),
        };
        assert!(infer_expression_type(&expression).is_err());
    }
    #[test]
    fn expression_addition_overflow_is_rejected() {
        let expression = Expression::Binary {
            left: Box::new(money("92233720368547758.07", "USD")),
            operator: BinaryOperator::Add,
            right: Box::new(money("0.01", "USD")),
        };
        let error = expression_amount(&expression).unwrap_err();
        assert!(error.contains("FP2009 ARITHMETIC_OVERFLOW"));
    }
    #[test]
    fn money_subtraction_overflow_is_rejected() {
        let left = MoneyAmount::from_minor_units(i64::MIN);
        let right = MoneyAmount::from_minor_units(1);
        assert!(left.checked_sub(right).is_err());
    }

    #[test]
    fn program_accepts_balanced_multiple_entries() {
        let mut declarations = base_program();
        declarations.push(Declaration::Transaction(Transaction {
            name: "Sale".to_string(),
            statements: vec![
                Statement::Debit(crate::ast::Debit {
                    account: "Cash".to_string(),
                    amount: money("100", "USD"),
                }),
                Statement::Credit(crate::ast::Credit {
                    account: "Revenue".to_string(),
                    amount: money("60", "USD"),
                }),
                Statement::Credit(crate::ast::Credit {
                    account: "Revenue".to_string(),
                    amount: money("40", "USD"),
                }),
            ],
        }));
        assert!(check_program(&Program { declarations }).is_ok());
    }
    #[test]
    fn program_rejects_duplicate_currency() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn program_rejects_duplicate_account() {
        let mut declarations = base_program();
        declarations.push(Declaration::Account(AccountDeclaration {
            name: "Cash".to_string(),
            account_type: AstAccountType::Asset,
            currency: "USD".to_string(),
            initial_balance: MoneyAmount::from_minor_units(0),
        }));
        assert!(check_program(&Program { declarations }).is_err());
    }
    #[test]
    fn program_rejects_unknown_account_currency() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Cash".to_string(),
                    account_type: AstAccountType::Asset,
                    currency: "EUR".to_string(),
                    initial_balance: MoneyAmount::from_minor_units(0),
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn program_accepts_transfer_with_compound_amount() {
        let mut declarations = vec![
            Declaration::Currency(CurrencyDeclaration {
                code: "USD".to_string(),
            }),
            Declaration::Account(AccountDeclaration {
                name: "A".to_string(),
                account_type: AstAccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(10000),
            }),
            Declaration::Account(AccountDeclaration {
                name: "B".to_string(),
                account_type: AstAccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: MoneyAmount::from_minor_units(0),
            }),
        ];
        declarations.push(Declaration::Transaction(Transaction {
            name: "Move".to_string(),
            statements: vec![Statement::Transfer(Transfer {
                amount: Expression::Binary {
                    left: Box::new(money("25", "USD")),
                    operator: BinaryOperator::Add,
                    right: Box::new(money("25", "USD")),
                },
                from: "A".to_string(),
                to: "B".to_string(),
            })],
        }));
        assert!(check_program(&Program { declarations }).is_ok());
    }
    #[test]
    fn program_rejects_payment_with_wrong_expression_currency() {
        let program = Program {
            declarations: vec![
                Declaration::Currency(CurrencyDeclaration {
                    code: "USD".to_string(),
                }),
                Declaration::Currency(CurrencyDeclaration {
                    code: "EUR".to_string(),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Customer".to_string(),
                    account_type: AstAccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: MoneyAmount::from_minor_units(10000),
                }),
                Declaration::Account(AccountDeclaration {
                    name: "Merchant".to_string(),
                    account_type: AstAccountType::Asset,
                    currency: "USD".to_string(),
                    initial_balance: MoneyAmount::from_minor_units(0),
                }),
                Declaration::Transaction(Transaction {
                    name: "Sale".to_string(),
                    statements: vec![Statement::Pay(Payment {
                        amount: money("10", "EUR"),
                        from: "Customer".to_string(),
                        to: "Merchant".to_string(),
                    })],
                }),
            ],
        };
        assert!(check_program(&program).is_err());
    }
    #[test]
    fn double_entry_balance_is_per_currency() {
        let mut debits = HashMap::new();
        let mut credits = HashMap::new();
        debits.insert(
            "USD".to_string(),
            MoneyAmount::from_decimal_str("100").unwrap(),
        );
        credits.insert(
            "USD".to_string(),
            MoneyAmount::from_decimal_str("100").unwrap(),
        );
        debits.insert(
            "EUR".to_string(),
            MoneyAmount::from_decimal_str("50").unwrap(),
        );
        credits.insert(
            "EUR".to_string(),
            MoneyAmount::from_decimal_str("50").unwrap(),
        );
        assert!(validate_double_entry_balance("MultiCurrency", &debits, &credits).is_ok());
    }
}
