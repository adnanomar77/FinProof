currency USD

account Customer: asset USD = 100
account Merchant: asset USD = 0

transaction Sale {
    pay 100 USD - 150 USD
    from Customer
    to Merchant
}
