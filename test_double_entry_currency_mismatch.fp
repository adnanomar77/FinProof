currency USD
currency EUR

account CashUSD: asset USD = 0
account SalesRevenue: revenue USD = 0

transaction CurrencyMismatch {
    debit CashUSD 100 EUR
    credit SalesRevenue 100 USD
}
