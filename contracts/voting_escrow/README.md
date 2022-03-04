# Voting Escrow

The veANC contract allows staking ANC to gain voting power. Voting power depends on the time the user is locking for.
Maximum lock time is 2 years which equals to 2.5 coefficient. For example, if the user locks 100 ANC for 2 years he
gains 250 voting power. Voting power is linearly decreased by passed periods. One period equals to 1 week.

## InstantiateMsg

```json
{
  "owner": "terra...",
  "anchor_token": "terra..."
}
```

## ExecuteMsg

### `receive`

Create new lock, extend current lock's amount or deposit on behalf other address.

```json
{
  "receive": {
    "sender": "terra...",
    "amount": "123",
    "msg": "<base64_encoded_json_string>"
  }
}
```

### `extend_lock_time`

Extends lock time by 1 week.

```json
{
  "extend_lock_time": {
    "time": 604800
  }
}
```

### `withdraw`

Withdraws whole amount of veANC if lock is expired.

```json
{
  "withdraw": {}
}
```

## Receive Hooks

### `create_lock`

Creates new lock for 'sender'.

```json
{
  "create_look": {
    "time": 604800
  }
}
```

### `deposit_for`

Deposits 'amount' (provided in `cw20 msg`) to user's lock.

```json
{
  "deposit_for": {
    "user": "terra..."
  }
}
```

### `extend_lock_amount`

Extends lock for 'sender' by specified 'amount' (provided in `cw20 msg`).

```json
{
  "extend_lock_amount": {}
}
```

## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `total_voting_power`

Returns total voting power at the current block period.

Response:

```json
{
  "voting_power_response": {
    "voting_power": 100
  }
}
```

### `user_voting_power`

Returns user's voting power at the current block period.

Request:

```json
{
  "user_voting_power": {
    "user": "terra..."
  }
}
```

Response:

```json
{
  "voting_power_response": {
    "voting_power": 10
  }
}
```

### `total_voting_power_at`

Returns total voting power at the specific time (in seconds).

Request:

```json
{
  "total_voting_power_at": {
    "time": 1234567
  }
}
```

Response:

```json
{
  "voting_power_response": {
    "voting_power": 10
  }
}
```

### `user_voting_power_at`

Returns user's voting power at the specific time (in seconds).

Request:

```json
{
  "user_voting_power_at": {
    "user": "terra...",
    "time": 1234567
  }
}
```

Response:

```json
{
  "voting_power_response": {
    "voting_power": 10
  }
}
```

### `get_last_user_slope`

Gets the most recently recorded rate of voting power decrease for 'user'.

Request:

```json
{
  "get_last_user_slope": {
    "user": "tera..."
  }
}
```

Response:

```json
{
  "slope": 2
}
```

### `lock_info`

Returns user's lock information.

Request:

```json
{
  "lock_info": {
    "user": "terra..."
  }
}
```

Response:

```json
{
  "lock_info_response": {
    "amount": 10,
    "coefficient": 2.5,
    "start": 2600,
    "end": 2704
  }
}
```

### `config`

Returns contract's config.

```json
{
  "config_response": {
    "owner": "terra...",
    "anchor_token": "terra..."
  }
}
```
