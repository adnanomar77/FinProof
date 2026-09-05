currency USD

account Customer: asset USD = 100
account Merchant: asset USD = 0

transaction Sale {
    pay 60 USD
    from Customer
    to Merchant
}

transaction FailedSale {
    pay 100 USD
    from Customer
    to Merchant
}
