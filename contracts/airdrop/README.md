# Anchor Airdrop <!-- omit in toc -->

## Init
Instantiate airdrop contract by registering owner and anchor token 

```
{
    "owner": "terra...",
    "anchor_token": "terra...",
}
```

## Register root
Register Merkle root hash for new airdrop round.

```
{
    "register_merkle_root": {
        "merkle_root": "634de21cde1044f41d90373733b0f0fb1c1c71f9652b905cdf159e73c4cf0d37"
    }
}
```

## Claim
Claim airdrop with proofs

```
{
    "claim": {
        "stage": 1,
        "amount": "1000000",
        "proof": [
            "ca2784085f944e5594bb751c3237d6162f7c2b24480b3a37e9803815b7a5ce42",
            "5b07b5898fc9aa101f27344dab0737aede6c3aa7c9f10b4b1fda6d26eb669b0f",
            "4847b2b9a6432a7bdf2bdafacbbeea3aab18c524024fc6e1bc655e04cbc171f3",
            "cad1958c1a5c815f23450f1a2761a5a75ab2b894a258601bf93cd026469d42f2"
        ]
    }
}
```