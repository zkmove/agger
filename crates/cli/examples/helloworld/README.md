###  Example

run the commands in aptos devnet

deploy:


``` bash
# build example contracts.
move build -d
# generate deployment transaction payload for aptos
agger-cli build-aptos-deployment-file -m helloworld --agger 0xb3a402b1d4c7797b3cb5c8ea77e4a2ca5fd7fb9b8def568d4339a50fc387aa59
# use aptos to submit the transaction
aptos move run --json-file deployments/aptos/helloworld.json

# build a query transaction payload for aptos
agger-cli build-query --function-id 0x1234::helloworld::say_he --agger 0xb3a402b1d4c7797b3cb5c8ea77e4a2ca5fd7fb9b8def568d4339a50fc387aa59

# use aptos to submit the transaction
aptos move run --json-file queries/aptos/helloworld/say_he.json
```

