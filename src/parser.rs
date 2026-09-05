use crate::ast::{
    AccountDeclaration, AccountType, BinaryOperator, Credit, CurrencyDeclaration, Debit,
    Declaration, Expression, MoneyLiteral, Payment, Program, Statement, Transaction, Transfer,
};

use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<Program, String> {
        let mut declarations = Vec::new();

        while !self.check(&TokenKind::Eof) {
            declarations.push(self.declaration()?);
        }

        Ok(Program { declarations })
    }

    // =================================================
    // Declarations
    // =================================================

    fn declaration(&mut self) -> Result<Declaration, String> {
        if self.match_kind(&TokenKind::Currency) {
            return Ok(Declaration::Currency(self.currency_declaration()?));
        }

        if self.match_kind(&TokenKind::Account) {
            return Ok(Declaration::Account(self.account_declaration()?));
        }

        if self.match_kind(&TokenKind::Transaction) {
            return Ok(Declaration::Transaction(self.transaction()?));
        }

        Err(self.error("Expected 'currency', 'account', or 'transaction'."))
    }

    // =================================================
    // Currency
    // =================================================

    fn currency_declaration(&mut self) -> Result<CurrencyDeclaration, String> {
        let code = self.consume_identifier("Expected currency code.")?;

        Ok(CurrencyDeclaration { code })
    }

    // =================================================
    // Account
    // =================================================

    fn account_declaration(&mut self) -> Result<AccountDeclaration, String> {
        let name = self.consume_identifier("Expected account name.")?;

        self.consume(&TokenKind::Colon, "Expected ':' after account name.")?;

        let account_type = self.account_type()?;

        let currency = self.consume_identifier("Expected currency after account type.")?;

        let initial_balance = if self.match_kind(&TokenKind::Equal) {
            let amount = self.consume(&TokenKind::Number, "Expected initial balance after '='.")?;

            crate::types::MoneyAmount::from_decimal_str(&amount.lexeme)
                .map_err(|_| self.error("Invalid initial account balance."))?
        } else {
            crate::types::MoneyAmount::from_minor_units(0)
        };

        if initial_balance.minor_units() < 0 {
            return Err(self.error("Initial account balance cannot be negative."));
        }

        Ok(AccountDeclaration {
            name,
            account_type,
            currency,
            initial_balance,
        })
    }

    fn account_type(&mut self) -> Result<AccountType, String> {
        if self.match_kind(&TokenKind::Asset) {
            return Ok(AccountType::Asset);
        }

        if self.match_kind(&TokenKind::Liability) {
            return Ok(AccountType::Liability);
        }

        if self.match_kind(&TokenKind::Equity) {
            return Ok(AccountType::Equity);
        }

        if self.match_kind(&TokenKind::Revenue) {
            return Ok(AccountType::Revenue);
        }

        if self.match_kind(&TokenKind::Expense) {
            return Ok(AccountType::Expense);
        }

        Err(self.error("Expected account type: Asset, Liability, Equity, Revenue, or Expense."))
    }

    // =================================================
    // Transaction
    // =================================================

    fn transaction(&mut self) -> Result<Transaction, String> {
        let name = self.consume_identifier("Expected transaction name.")?;

        self.consume(
            &TokenKind::LeftBrace,
            "Expected '{' after transaction name.",
        )?;

        let mut statements = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.check(&TokenKind::Eof) {
            statements.push(self.statement()?);
        }

        self.consume(&TokenKind::RightBrace, "Expected '}' after transaction.")?;

        Ok(Transaction { name, statements })
    }

    // =================================================
    // Statements
    // =================================================

    fn statement(&mut self) -> Result<Statement, String> {
        if self.match_kind(&TokenKind::Pay) {
            return Ok(Statement::Pay(self.payment()?));
        }

        if self.match_kind(&TokenKind::Transfer) {
            return Ok(Statement::Transfer(self.transfer()?));
        }

        if self.match_kind(&TokenKind::Debit) {
            return Ok(Statement::Debit(self.debit()?));
        }

        if self.match_kind(&TokenKind::Credit) {
            return Ok(Statement::Credit(self.credit()?));
        }

        Err(self.error("Expected financial operation."))
    }

    // =================================================
    // Pay
    // =================================================

    fn payment(&mut self) -> Result<Payment, String> {
        let amount = self.expression()?;

        self.consume(&TokenKind::From, "Expected 'from' after payment amount.")?;

        let from = self.consume_identifier("Expected source account.")?;

        self.consume(&TokenKind::To, "Expected 'to' after source account.")?;

        let to = self.consume_identifier("Expected destination account.")?;

        Ok(Payment { amount, from, to })
    }

    // =================================================
    // Transfer
    // =================================================

    fn transfer(&mut self) -> Result<Transfer, String> {
        let amount = self.expression()?;

        self.consume(&TokenKind::From, "Expected 'from' after transfer amount.")?;

        let from = self.consume_identifier("Expected source account.")?;

        self.consume(&TokenKind::To, "Expected 'to' after source account.")?;

        let to = self.consume_identifier("Expected destination account.")?;

        Ok(Transfer { amount, from, to })
    }

    // =================================================
    // Double-Entry
    // =================================================

    fn debit(&mut self) -> Result<Debit, String> {
        let account = self.consume_identifier("Expected account after 'debit'.")?;
        let amount = self.expression()?;

        Ok(Debit { account, amount })
    }

    fn credit(&mut self) -> Result<Credit, String> {
        let account = self.consume_identifier("Expected account after 'credit'.")?;
        let amount = self.expression()?;

        Ok(Credit { account, amount })
    }
    // =================================================
    // Expressions
    // =================================================

    fn expression(&mut self) -> Result<Expression, String> {
        self.additive_expression()
    }

    fn additive_expression(&mut self) -> Result<Expression, String> {
        let mut expression = self.primary()?;

        loop {
            let operator = if self.match_kind(&TokenKind::Plus) {
                Some(BinaryOperator::Add)
            } else if self.match_kind(&TokenKind::Minus) {
                Some(BinaryOperator::Subtract)
            } else {
                None
            };

            let Some(operator) = operator else {
                break;
            };

            let right = self.primary()?;

            expression = Expression::Binary {
                left: Box::new(expression),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expression)
    }

    fn primary(&mut self) -> Result<Expression, String> {
        if self.match_kind(&TokenKind::LeftParen) {
            let expression = self.expression()?;

            self.consume(&TokenKind::RightParen, "Expected ')' after expression.")?;

            return Ok(expression);
        }

        let amount = self.consume(&TokenKind::Number, "Expected money amount.")?;

        let currency = self.consume_identifier("Expected currency after amount.")?;

        Ok(Expression::Money(MoneyLiteral {
            amount: amount.lexeme,
            currency,
        }))
    }

    // =================================================
    // Token Helpers
    // =================================================

    fn consume_identifier(&mut self, message: &str) -> Result<String, String> {
        if self.check(&TokenKind::Identifier) {
            let token = self.advance().clone();
            return Ok(token.lexeme);
        }

        Err(self.error(message))
    }

    fn consume(&mut self, kind: &TokenKind, message: &str) -> Result<Token, String> {
        if self.check(kind) {
            return Ok(self.advance().clone());
        }

        Err(self.error(message))
    }

    fn match_kind(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek().kind == *kind
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }

        &self.tokens[self.current - 1]
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn is_at_end(&self) -> bool {
        self.check(&TokenKind::Eof)
    }

    fn error(&self, message: &str) -> String {
        let token = self.peek();

        format!(
            "FP4001 PARSER_ERROR: Parser error at line {}, column {}: {}",
            token.line, token.column, message
        )
    }
}

// =====================================================
// Tests
// =====================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse_source(source: &str) -> Program {
        let mut lexer = Lexer::new(source);

        let tokens = lexer.tokenize().unwrap();

        let mut parser = Parser::new(tokens);

        parser.parse().unwrap()
    }

    #[test]
    fn parses_currency() {
        let program = parse_source(
            r#"
                currency USD
            "#,
        );

        assert_eq!(
            program.declarations,
            vec![Declaration::Currency(CurrencyDeclaration {
                code: "USD".to_string(),
            })]
        );
    }

    #[test]
    fn parses_asset_account() {
        let program = parse_source(
            r#"
                account Cash: Asset USD
            "#,
        );

        assert_eq!(
            program.declarations,
            vec![Declaration::Account(AccountDeclaration {
                name: "Cash".to_string(),
                account_type: AccountType::Asset,
                currency: "USD".to_string(),
                initial_balance: crate::types::MoneyAmount::from_minor_units(0),
            })]
        );
    }

    #[test]
    fn parses_liability_account() {
        let program = parse_source(
            r#"
                account Loan: Liability USD
            "#,
        );

        assert_eq!(
            program.declarations,
            vec![Declaration::Account(AccountDeclaration {
                name: "Loan".to_string(),
                account_type: AccountType::Liability,
                currency: "USD".to_string(),
                initial_balance: crate::types::MoneyAmount::from_minor_units(0),
            })]
        );
    }

    #[test]
    fn parses_accounts_and_transaction_together() {
        let program = parse_source(
            r#"
                account Cash: Asset USD
                account Loan: Liability USD

                transaction Payment {
                    pay 100 USD
                    from Cash
                    to Loan
                }
            "#,
        );

        assert_eq!(program.declarations.len(), 3);

        match &program.declarations[0] {
            Declaration::Account(account) => {
                assert_eq!(account.name, "Cash");
                assert_eq!(account.account_type, AccountType::Asset);
                assert_eq!(account.currency, "USD");
            }

            _ => panic!("Expected account"),
        }

        match &program.declarations[1] {
            Declaration::Account(account) => {
                assert_eq!(account.name, "Loan");
                assert_eq!(account.account_type, AccountType::Liability);
                assert_eq!(account.currency, "USD");
            }

            _ => panic!("Expected account"),
        }

        match &program.declarations[2] {
            Declaration::Transaction(transaction) => {
                assert_eq!(transaction.name, "Payment");
            }

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn rejects_invalid_account_type() {
        let source = r#"
            account Cash: Unknown USD
        "#;

        let mut lexer = Lexer::new(source);

        let tokens = lexer.tokenize().unwrap();

        let mut parser = Parser::new(tokens);

        assert!(parser.parse().is_err());
    }

    #[test]
    fn parses_money_literal() {
        let program = parse_source(
            r#"
                transaction Sale {
                    pay 1000 USD
                    from Customer
                    to Merchant
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => match &transaction.statements[0] {
                Statement::Pay(payment) => {
                    assert_eq!(
                        payment.amount,
                        Expression::Money(MoneyLiteral {
                            amount: "1000".to_string(),
                            currency: "USD".to_string(),
                        })
                    );
                }

                Statement::Transfer(_) => {
                    panic!("Expected pay statement");
                }

                Statement::Debit(_) | Statement::Credit(_) => {
                    panic!("Expected pay statement");
                }
            },

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn parses_money_addition() {
        let program = parse_source(
            r#"
                transaction Sale {
                    pay 100 USD + 50 USD
                    from Customer
                    to Merchant
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => match &transaction.statements[0] {
                Statement::Pay(payment) => match &payment.amount {
                    Expression::Binary { operator, .. } => {
                        assert_eq!(operator, &BinaryOperator::Add);
                    }

                    _ => panic!("Expected binary expression"),
                },

                Statement::Transfer(_) => {
                    panic!("Expected pay statement");
                }

                Statement::Debit(_) | Statement::Credit(_) => {
                    panic!("Expected pay statement");
                }
            },

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn parses_parenthesized_expression() {
        let program = parse_source(
            r#"
                transaction Sale {
                    pay (100 USD + 50 USD)
                    from Customer
                    to Merchant
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => match &transaction.statements[0] {
                Statement::Pay(payment) => match &payment.amount {
                    Expression::Binary { operator, .. } => {
                        assert_eq!(operator, &BinaryOperator::Add);
                    }

                    _ => panic!("Expected binary expression"),
                },

                Statement::Transfer(_) => {
                    panic!("Expected pay statement");
                }

                Statement::Debit(_) | Statement::Credit(_) => {
                    panic!("Expected pay statement");
                }
            },

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn parses_nested_parentheses() {
        let program = parse_source(
            r#"
                transaction Sale {
                    pay ((100 USD + 50 USD) - 25 USD)
                    from Customer
                    to Merchant
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => match &transaction.statements[0] {
                Statement::Pay(payment) => match &payment.amount {
                    Expression::Binary { operator, .. } => {
                        assert_eq!(operator, &BinaryOperator::Subtract);
                    }

                    _ => panic!("Expected binary expression"),
                },

                Statement::Transfer(_) => {
                    panic!("Expected pay statement");
                }

                Statement::Debit(_) | Statement::Credit(_) => {
                    panic!("Expected pay statement");
                }
            },

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn parses_debit_and_credit() {
        let program = parse_source(
            r#"
                transaction Sale {
                    debit Cash 100 USD
                    credit SalesRevenue 100 USD
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => {
                match &transaction.statements[0] {
                    Statement::Debit(debit) => {
                        assert_eq!(debit.account, "Cash");

                        assert_eq!(
                            debit.amount,
                            Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            })
                        );
                    }

                    _ => panic!("Expected debit statement"),
                }

                match &transaction.statements[1] {
                    Statement::Credit(credit) => {
                        assert_eq!(credit.account, "SalesRevenue");

                        assert_eq!(
                            credit.amount,
                            Expression::Money(MoneyLiteral {
                                amount: "100".to_string(),
                                currency: "USD".to_string(),
                            })
                        );
                    }

                    _ => panic!("Expected credit statement"),
                }
            }

            _ => panic!("Expected transaction"),
        }
    }
    #[test]
    fn parses_transfer() {
        let program = parse_source(
            r#"
                transaction InternalTransfer {
                    transfer 250 USD
                    from Cash
                    to Savings
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => match &transaction.statements[0] {
                Statement::Transfer(transfer) => {
                    assert_eq!(
                        transfer.amount,
                        Expression::Money(MoneyLiteral {
                            amount: "250".to_string(),
                            currency: "USD".to_string(),
                        })
                    );

                    assert_eq!(transfer.from, "Cash");

                    assert_eq!(transfer.to, "Savings");
                }

                Statement::Pay(_) => {
                    panic!("Expected transfer statement");
                }

                Statement::Debit(_) | Statement::Credit(_) => {
                    panic!("Expected transfer statement");
                }
            },

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn parses_transfer_with_addition() {
        let program = parse_source(
            r#"
                transaction InternalTransfer {
                    transfer 100 USD + 50 USD
                    from Cash
                    to Savings
                }
            "#,
        );

        match &program.declarations[0] {
            Declaration::Transaction(transaction) => match &transaction.statements[0] {
                Statement::Transfer(transfer) => match &transfer.amount {
                    Expression::Binary { operator, .. } => {
                        assert_eq!(operator, &BinaryOperator::Add);
                    }

                    _ => panic!("Expected binary expression"),
                },

                Statement::Pay(_) => {
                    panic!("Expected transfer statement");
                }

                Statement::Debit(_) | Statement::Credit(_) => {
                    panic!("Expected transfer statement");
                }
            },

            _ => panic!("Expected transaction"),
        }
    }

    #[test]
    fn rejects_missing_transfer_destination() {
        let source = r#"
            transaction InternalTransfer {
                transfer 250 USD
                from Cash
            }
        "#;

        let mut lexer = Lexer::new(source);

        let tokens = lexer.tokenize().unwrap();

        let mut parser = Parser::new(tokens);

        assert!(parser.parse().is_err());
    }

    #[test]
    fn rejects_missing_destination() {
        let source = r#"
            transaction Sale {
                pay 1000 USD
                from Customer
            }
        "#;

        let mut lexer = Lexer::new(source);

        let tokens = lexer.tokenize().unwrap();

        let mut parser = Parser::new(tokens);

        assert!(parser.parse().is_err());
    }

    #[test]
    fn rejects_missing_closing_parenthesis() {
        let source = r#"
            transaction Sale {
                pay (100 USD + 50 USD
                from Customer
                to Merchant
            }
        "#;

        let mut lexer = Lexer::new(source);

        let tokens = lexer.tokenize().unwrap();

        let mut parser = Parser::new(tokens);

        assert!(parser.parse().is_err());
    }
}
