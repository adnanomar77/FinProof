currency USD

account Customer: asset USD = 500
account Merchant: asset USD = 0
account Bank: asset USD = 0

transaction Sale {
    pay 100 USD
    from Customer
    to Merchant
}

transaction Settlement {
    transfer 60 USD
    from Merchant
    to Bank
}
