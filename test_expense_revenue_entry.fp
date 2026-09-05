currency USD

account ExpenseAccount: expense USD = 0
account SalesRevenue: revenue USD = 0

transaction ExpenseEntry {
    debit ExpenseAccount 40 USD
    credit SalesRevenue 40 USD
}
