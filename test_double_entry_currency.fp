currency USD
currency EUR

account Cash: asset USD = 0
account SalesRevenue: revenue USD = 0

transaction CurrencyMismatch {
    debit Cash 100 EUR
    credit SalesRevenue 100 EUR
}
