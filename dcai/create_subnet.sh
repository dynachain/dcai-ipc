#!/bin/bash

cargo run -q -p ipc-cli --release -- subnet create --parent /r314159 \
--min-validator-stake 100 --min-validators 3 \
--bottomup-check-period 300 \
--from 0x5b34687030fb33a0d0ae91209c6c95708f520904 \
--permission-mode collateral \
--supply-source-kind erc20 \
--supply-source-address 0xCe3E3652A654991790ceB0b1e717b1ed82139112 \
--collateral-source-kind erc20 --collateral-source-address 0x4C2CdB7B8A18a607B7df366ddF3A78bda12fEad2 \
--active-validators-limit 60000


#created subnet actor with id:  
# /r314159/t410f6qldtvezeucezoaozkz2gg2ludaf3s77tryboya
