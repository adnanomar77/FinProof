currency USD

account Cash: asset USD = 100
account Payable: liability USD = 100
account SalesRevenue: revenue USD = 100
account OperatingExpense: expense USD = 100

transaction ReverseDirections {
    debit Payable 20 USD
    debit SalesRevenue 20 USD
    credit Cash 20 USD
    credit OperatingExpense 20 USD
}
