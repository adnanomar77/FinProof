currency USD

account Cash: asset USD = 100
account Equipment: asset USD = 0
account SalesRevenue: revenue USD = 0

transaction RuntimeRollback {
    debit Equipment 50 USD
    credit SalesRevenue 50 USD
    debit SalesRevenue 200 USD
    credit Cash 200 USD
}
