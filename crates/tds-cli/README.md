### Run Example

run the commands in aptos devnet
``` shell
cd examples/helloworld
move build -d
tds-cli build-aptos-deployment-file -m helloworld --tds 0xb3a402b1d4c7797b3cb5c8ea77e4a2ca5fd7fb9b8def568d4339a50fc387aa59
aptos move run --json-file deployments/aptos/helloworld.json
```

