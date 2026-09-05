currency USD

account Cash: asset USD = 100
account Payable: liability USD = 100
account OwnerEquity: equity USD = 100

transaction ReduceBalances {
    debit Payable 50 USD
    debit OwnerEquity 50 USD
    credit Cash 100 USD
}
