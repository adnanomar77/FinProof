currency USD

account Customer: customer USD = 100
account SalesRevenue: revenue USD = 0

transaction InvalidDebit {
    debit Customer 40 USD
    credit SalesRevenue 40 USD
}
