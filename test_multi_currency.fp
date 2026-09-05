currency USD
currency EUR

account CashUSD: asset USD = 0
account CashEUR: asset EUR = 0
account RevenueUSD: revenue USD = 0
account RevenueEUR: revenue EUR = 0

transaction MultiCurrency {
    debit CashUSD 100 USD
    credit RevenueUSD 100 USD
    debit CashEUR 200 EUR
    credit RevenueEUR 200 EUR
}
