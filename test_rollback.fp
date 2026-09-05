currency USD

account Customer: asset USD = 100
account Merchant: asset USD = 0

transaction AtomicSale {
    pay 60 USD
    from Customer
    to Merchant

    pay 60 USD
    from Customer
    to Merchant
}
