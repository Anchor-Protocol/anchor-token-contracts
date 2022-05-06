# Voting Escrow

The veANC contract allows locking sANC (staked ANC) to gain voting power. Voting power depends on the amount of time the user is locking for.
Maximum lock time is 4 years which equals to 2.5 coefficient. For example, if the user locks 100 sANC for 4 years he
gains 250 voting power. Voting power is linearly decreased by passed periods. One period equals to 1 week.

## InstantiateMsg

```json
{
  "owner": "terra...",
  "anchor_token": "terra...",
  "min_lock_time": 2419200,
  "max_lock_time": 62899200,
  "period_duration": 604800,
  "boost_coefficient": 25
}
```

## ExecuteMsg

### `extend_lock_amount`

Extends lock amount for specified user.

```json
{
  "extend_lock_amount": {
    "user": "terra...",
    "amount": "500"
  }
}
```

### `extend_lock_time`

Extends lock time by 1 week for specified user.

```json
{
  "extend_lock_time": {
    "user": "terra...",
    "time": 604800
  }
}
```

### `withdraw`

Withdraws specified amount of veANC for user if lock has expired.

```json
{
  "withdraw": {
    "user": "terra...",
    "amount": "500"
  }
}
```
## QueryMsg

All query messages are described below. A custom struct is defined for each query response.

### `total_voting_power`

Returns total voting power at the current block period.

Request 
```json
{
  "total_voting_power": {}
}
```

Response:

```json
{
  "voting_power": 100
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
  "voting_power": 10
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
  "voting_power": "10"
}
```

### `total_voting_power_at_period`

Returns total voting power at the specific period (in weeks).

Request:

```json
{
  "total_voting_power_at_period": {
    "period": 2052
  }
}
```

Response:

```json
{
  "voting_power": "10"
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
  "voting_power": "10"
}
```

### `user_voting_power_at_period`

Returns user's voting power at the specific period (in weeks).

Request:

```json
{
  "user_voting_power_at_period": {
    "user": "terra...",
    "period": 2052
  }
}
```

Response:

```json
{
  "voting_power": "10"
}
```

### `last_user_slope`

Gets the most recently recorded rate of voting power decrease for 'user'.

Request:

```json
{
  "last_user_slope": {
    "user": "tera..."
  }
}
```

Response:

```json
{
  "slope": "2.0"
}
```

### `user_unlock_period`

Returns user's unlock period.

Request:

```json
{
  "user_unlock_period": {
    "user": "tera..."
  }
}
```

Response:

```json
{
  "unlock_period": 2052
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
  "amount": "10",
  "coefficient": "2.5",
  "start": 2600,
  "end": 2704
}
```

### `config`

Returns contract's config.

```json
{
  "owner": "terra...",
  "anchor_token": "terra...",
  "min_lock_time": 2419200,
  "max_lock_time": 62899200,
  "period_duration": 604800,
  "boost_coefficient": 25
}
```
