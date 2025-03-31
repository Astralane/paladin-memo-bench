# Paladin Memo Bench

A benchmark tool to assess performance of the paladin port.
Uses Astralane's Paladin leaders trackers and sends memo transactions to them at the start of their first leader slot. 

## Environment Variables

Before running the program, ensure you set the following environment variables:

- `WS_RPC` - The WebSocket RPC endpoint.  
  *Example:* `ws://rpc:8900`

- `HTTP_RPC` - The HTTP RPC endpoint.  
  *Example:* `http://rpc:8899`

- `PALADIN_RPC` - The RPC endpoint for the Paladin client.  
  *Example:* `http://client-rpc:4041`

- `KEYPAIR_PATH` - Path to your Solana keypair file.  
  *Example:* `/Users/nuel/.config/solana/id.json`

- `duration_mins` - The total time (in minutes) the benchmark runs.  
  *Example:* `1`

- `CU_PRICE_MICRO_LAMPORTS` - The cost per compute unit in micro lamports.  
  *Example:* `100000000`
