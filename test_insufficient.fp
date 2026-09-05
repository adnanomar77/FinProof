currency USD

account Customer: asset USD = 50
account Merchant: asset USD = 0

transaction Sale {
    pay 100 USD
    from Customer
    to Merchant
}
