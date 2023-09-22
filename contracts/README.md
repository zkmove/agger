# agger-contracts

aptos move contracts for agger

### build

``` shell
aptos move compile --named-addresses agger=0x1234 --skip-fetch-latest-git-deps
```

### deploy

first you need to fund your account with `aptos account fund-with-faucet`, then create resource account and publish packages.

```shell
aptos move create-resource-account-and-publish-package --address-name agger --seed agger --skip-fetch-latest-git-deps
```