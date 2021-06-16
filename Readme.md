# Sgxtop

This crate is top-like tool,intended for inter SGX performance monitoring.

Your need a customized [linux-sgx-driver](https://github.com/lengyijun/linux-sgx-driver/tree/top) first.

## How to use?
View on [asciinema](https://asciinema.org/a/UMI3BqI7wTScXuPSpneJiH4Sv)

`/proc/sgx_stats` collect global information, such as:
- eadd/eremove/ewb/eldu speed
- ewb/eldu total count
- total/free/used EPC memory
- VA pages used

`/proc/sgx_enclaves` collect information per enclave, such as:
- enclave id (global unique)
- process id and process command
- eadd count
- RSS memory (in EPC)
- swapped out memory
- enclave state
- enclave uptime

