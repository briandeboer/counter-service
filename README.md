# Counter service

The counter service receives logged events and puts them into time windowed buckets based on the configuration for a specific application. It supports both GraphQL and REST endpoints. The items are batch logged into the database on a regular interval which can be specified in the environment variables. Currently there is no archival of old data but that is likely an improvement which should be made further down the road.

The service _should be_ performant enough to put on the front line of your requests to receive all of the events, however based on your traffic and/or number of events that are coming in, as well as the number of groupings that you are enabling, it might make more sense to stream the events into something else and then log the events separately to the counter service so that you can control any spikyness or other factors.

## Configuration

The counter service will look for any configurations objects which define how the data is grouped. The grouping effects the secondary index of the items (See below). A configuration object looks like:

```Graphql
type Config {
  applicationId: ID
  buckets: [HOUR, DAY] # possible values are HOUR, DAY, WEEK, MONTH, ALL_TIME
  groups: [
    "eventType|campaignId",
    "eventType|campaignId|ipAddress",
  ]
  logAllEvents: false # default is false
}
```

If logAllEvents is then all the raw events will be logged into an `<application_id>_all` group. Otherwise they are only embedded.

### Important

- In the keys the order does not matter.

- All keys (and values) are always all lowercase when stored in the database to make them case-insensitive. Queries with ids are also lowercased in the request so you don't need to worry about remembering the casing.

- Requests made to log or query events to application that has not been configured will result in an error.

- Configuration is loaded one time at startup for the service. If you need to reconfigure or add configurations you will need to restart the application.

## Logging an event

Let's assume that you log three different events, two on the same IP address but with different user agents and one on a different IP.

```GraphQL
mutation LogEventA {
  logEvent(
    newEvent {
      applicationId: "appId"
      keys: [
        { key: "eventType", value: "click" },
        { key: "ipAddress", value: "1.2.3.4" },
        { key: "userAgent", value: "Some Very Long User Agent" },
        { key: "campaignId", value: "someValue" }
      ],
      timestamp: 100000000 # 9:48
    }
  ) {
    id
  }
}
```

and again, the same but with a different timestamp but in the next hour:

```GraphQL
mutation LogEventB {
  logEvent(
    newEvent {
      applicationId: "appId"
      keys: [
        { key: "eventType", value: "click" },
        { key: "ipAddress", value: "1.2.3.4" },
        { key: "userAgent", value: "Some Very Long User Agent" },
        { key: "campaignId", value: "someValue" }
      ],
      timestamp: 100001111 # 10:05
    }
  ) {
    id
  }
}
```

And finally another with a different timestamp and ip address:

```GraphQL
mutation LogEventC {
  logEvent(
    newEvent {
      applicationId: "appId",
      keys: [
        { key: "eventType", value: "click" },
        { key: "ipAddress", value: "2.3.4.5" },
        { key: "userAgent", value: "Some Very Long User Agent" },
        { key: "campaignId", value: "someValue" }
      ],
      timestamp: 100002222 # 10:23
    }
  ) {
    id
  }
}
```

An event will be logged into the all_events bucket, and based on the above example five buckets of data will be inserted as well...

### Bucket 1

Grouped by eventType, campaignId, and hour.
Collection name in mongo will be `events_bucket_hour`

```js
// two records because two different hours
// LogEventA
{
  hash: "click|somevalue|99997200", // last number is timestamp of 9:00am UTC
  applicationId, "appId",
  timestamp: 99997200,
  window: "Hour",
  grouping: "eventType|campaignId",
  nested_groups: [], //  nothing in groups yet because there are no subsets
  count: 1, // the total number of objects
  events: [
    { timestamp: 99997200, ipaddress: "1.2.3.4", ... }
  ]
},

// LogEventB & LogEventC
{
  hash: "click|somevalue|100000800", // last number is timestamp of 10:00am UTC
  applicationId, "appId",
  timestamp: 100000800,
  window: "Hour",
  grouping: "eventType|campaignId",
  nested_groups: [], //  nothing in groups yet because there are no subsets
  count: 2, // the total number of objects
  events: [
    { LogEventB ... },
    { LogEventC ... }
  ]
}
```

### Bucket 2

Grouped by eventType, campaignId, and day
Collection name in mongo will be `events_bucket_day`

```js
// All three events
{
  hash: "appid|click|somevalue|99964800", // timestamp of 00:00 UTC
  applicationId, "appId",
  timestamp: 99964800,
  window: "Day",
  grouping: "eventType|campaignId",
  nested_groups: [], //  nothing in groups yet because there are no subsets
  count: 3, // the total number of objects
  events: [
    { LogEventA ... },
    { LogEventB ... },
    { LogEventC ... }
  ]
}
```

### Bucket 3

Because the application config has another group `["eventType", "campaignId", "ipAddress"]`  items will get grouped and inserted again. And, since there is a group config which is a subset of it, that group will get inserted as well into the groups array. That will allow you later to check if there is repetition in the datasets.

Grouped by eventType, campaignId, ipAddress and hour
Collection name in mongo will be `events_bucket_hour`

```js
// three records because two different hours and two different ips
// LogEventA
{
  hash: "Hour|click|somevalue|1.2.3.4|99997200", // last number is timestamp of 9:00am UTC
  applicationId, "appId",
  timestamp: 99997200,
  window: "Hour",
  grouping: "eventType|campaignId|ipAddress",
  nested_groups: ["click|somevalue"],
  count: 1, // the total number of objects
  events: [
    { LogEventA ... },
  ]
},

// LogEventB
{
  hash: "Hour|click|somevalue|1.2.3.4|100000800", // last number is timestamp of 10:00am UTC
  applicationId, "appId",
  timestamp: 100000800,
  window: "Hour",
  grouping: "eventType|campaignId|ipAddress",
  nested_groups: ["click|somevalue"],
  count: 1, // the total number of objects
  events: [
    { LogEventB ... },
  ]
}

// LogEventC
{
  hash: "Hour|click|somevalue|2.3.4.5|100002222", // last number is timestamp of 10:00am UTC
  applicationId, "appId",
  timestamp: 100002222,
  window: "Hour",
  grouping: "eventType|campaignId|ipAddress",
  nested_groups: ["click|somevalue"],
  count: 1, // the total number of objects
  events: [
    { LogEventC ... },
  ]
}

Note: Notice how LogEventB and LogEventC have a shared group. That will allow you to make queries later.

```

### Bucket 4

Grouped by eventType, campaignId, ip address, and day
Collection name in mongo will be `events_bucket_day`

```js
// Two records because LogEventA & LogEventB get grouped together
{
  hash: "Day|click|somevalue|1.2.3.4|99964800", // timestamp of 00:00 UTC
  applicationId, "appId",
  timestamp: 99964800,
  window: "Day",
  grouping: "eventType|campaignId|ipAddress",
  nested_groups: ["click|somevalue"],
  count: 2, // the total number of objects
  events: [
    { LogEventA ... },
    { LogEventB ... },
  ]
},
// LogEventC
{
  hash: "Day|click|somevalue|2.3.4.5|99964800", // timestamp of 00:00 UTC
  applicationId, "appId",
  timestamp: 99964800,
  window: "Day",
  grouping: "eventType|campaignId|ipAddress",
  nested_groups: ["click|somevalue"],
  count: 1, // the total number of objects
  events: [
    { LogEventC ... },
  ]
}
```

## Retrieving Data

So, in order to find the number of unique (per IP) events that occurred in a day, we have multiple ways to accomplish that.

First, if we know exactly what we want we can just ask for it. We know we want the day represented by the timestamp `99964800` and the hash "click|somevalue" so retrieve it by the keys and look at the count.

```Graphql
query EventGroupByKeys {
  eventGroupByKeys(
    window: DAY,
    timestamp: 99964800,
    grouping: "eventType|campaignId",
    keys: [
      { key: "eventType", value: "click" },
      { key: "campaignId", value: "someValue" }
    ]) {
    count
  }
}
```

The returned count will be exactly the number of events that match. This is obviously the most efficient query because it is a single record by id. However, you will only be able to tell how many total events happened without knowing how many were the same ip address because they are not grouped.

Another option is that we can request the information by the grouping. (Note there are two different options here, `countByGroup` and `eventsByGroup`). `countByGroup` will solely count the items which is more efficient. `eventsByGroup` will return the actual data, but will require paginating to count up all of the inner items.

```Graphql
query CountByGroup {
  countByGroup(
    applicationId: "appId"
    grouping: "eventType|campaignId|ipAddress"
    nested_groupings: "click|somevalue"
    timestamp: 99964800
    window: DAY
  ) {
    recordCount
    aggregateCount
  }
}
```

The above query using `countByGroup` returns two data fields, `recordCount` and `aggregateCount`. `recordCount` is the total number of distinct records that were returned. In this example, that means how many records that match the "click|somevalue" grouping which would be 2 (there are two records in bucket 4). The `aggregateCount` looks inside each record and adds the `count` property. In this case that would be 3 (adding the count from Bucket 4). Unlike looking up the data by id, we can tell both the total count of clicks and the total number of distinct clicks per ip.

`eventGroups` is similar, while less efficient, but is really only useful to get the total unless you plan to loop through all the events to add the aggregate count yourself.

```Graphql
query EventsGroupedByIp {
  eventGroups(
    applicationId: "appId"
    window: DAY
    startTimestamp: 99964800
    endTimestamp: 100051200 # next day
    grouping: "eventType|campaignId|ipAddress"
    nested_grouping: "click|someValue"
  ) {
    totalCount
    items {
      id
      count
    }
    pageInfo {
      startCursor
      nextCursor
      hasNextPage
      hasPreviousPage
    }
  }
}
```

In the query above we get all of the events from bucket 4, but the totalCount would be `2` so we would know that there were two unique ips.

## Another use case

Let's take another use case... Assume we want to count the number of votes on a certain question and that users are restricted from voting more than once. We could use a config like so:

```js
{
  applicationId: 'voteapp',
  buckets: [ALL], // we don't actually care when they vote, so don't limit by time
  groups: [
    ["questionId"],
    ["questionId", "answerId"],
    // ["userId"], // if you want to see all votes by a user regardless of question
    ["questionId", "userId"] // if you want to limit to one per user
  ]
}
```

With the above, votes would get grouped together in three ways. Here are examples of the data that would be inserted into the `events_bucket_all` collection:

```js
{
  hash: "voteapp|question1|-1", // timestamp is -1 because it's all time
  applicationId, "appId",
  timestamp: -1,
  window: "AllTime",
  grouping: "questionId",
  nested_groups: [],
  count: 3, // the total number of votes for question1 (not limited by user)
  events: [
    { vote for question1/answer1/user1 },
    { vote for question1/answer2/user2 },
    { vote for question1/answer3/user2 },
    { vote for question1/answer1/user2 },
    { vote for question1/answer1/user1 },
  ]
},
{
  hash: "voteapp|question1|answer1|-1", // timestamp is -1 because it's all time
  applicationId, "appId",
  timestamp: -1,
  window: "AllTime",
  grouping: "questionId|answerId",
  nested_groups: ["question1"],
  count: 3, // the total number of votes for question1 and answer1
  events: [
    { vote for question1/answer1/user1 },
    { vote for question1/answer1/user2 },
    { vote for question1/answer1/user1 },
  ]
},
{
  hash: "voteapp|question1|user1|-1", // timestamp is -1 because it's all time
  applicationId, "appId",
  timestamp: -1,
  window: "AllTime",
  grouping: "questionId|userId",
  nested_groups: ["question1"],
  count: 2, // the total number of votes for question1 and answer1 for user 1
  events: [
    { vote for question1/answer1/user1 },
    { vote for question1/answer1/user1 },
  ]
},
{
  hash: "voteapp|question1|user2", // timestamp is -1 because it's all time
  applicationId, "appId",
  timestamp: -1,
  window: "AllTime",
  grouping: "questionId|userId",
  nested_groups: ["question1"],
  count:1, // the total number of votes for question1 and answer1 for user 2
  events: [
    { vote for question1/answer1/user2 },
  ]
},
...
```

If you wanted to limit the number of votes that each user can make to only 1 you could add the following grouping to the config:

```js
 ["questionId", "userId"] // if you want to limit to one per user
 ```

Then you could make a request by id for "questionid|userid" and if anything comes back you know that this user has already voted for this question. Then you can restrict it.

## Getting Started

- Install [Rust](https://www.rust-lang.org/tools/install)
- Run `cargo run` to build and run service

### Install Docker and Docker Compose (Optional)

- [Docker](https://docs.docker.com/engine/install/)
- [docker-compose](https://docs.docker.com/compose/install/)

### Start Mongo

You can skip this step if you prefer to run a local Mongo instance.

Using docker-compose

```sh
docker-compose up -d mongo
```

In the future you can run `docker start mongo` to relaunch the service.

## VSCode

### Plugins

- Better TOML
- Native Debug
- Rust
- rust-analyzer

### Debugging

Run debug (F5) to create a **`launch.json`** file, make sure the value of **target** is pointed to your built executable, usually `./target/rust-deps/debug/counter-service` unless you changed `$CARGO_TARGET_DIR`

## Graphiql

- [http://localhost:8084/counter-service/graphiql](http://localhost:8084/counter-service/graphiql)

## Docker

You can run the service with docker-compose. It currently doesn't take into consideration the login-service, but that is something to look into the best way to accomplish.

### Build

```sh
docker build -t counter-service:latest .
```

### Run

```sh
docker-compose up -d
```

## Tests

In order to run tests against graphql you will need to have a local runnning mongo server on port 27017. You can use docker-compose or whatever method you want to have that. It will automatically create a new database just for tests named `counter-service-test`. In order for your test to leverage the database you should configure your test app with the `load-filled-database`. That will read all mock data from the `tests/mock` folder and insert them into the database. Here's an example...

```rust
use crate::utils;

...

let mut app = test::init_service(
    App::new()
        .configure(utils::load_filled_database)
        .configure(app_routes),
).await;

```

For more information, look at the `schema/query/users.rs` tests. For most tests, using test snapshots will work well. For more information on snapshot testing see [here](https://jestjs.io/docs/en/snapshot-testing#best-practices). The tests rely on the `insta` [crate](https://github.com/mitsuhiko/insta). First install insta follow the instructions on their site:

```sh
cargo install cargo-insta
```

To run the tests first make sure that you have mongo running on local port 27084 or use docker-compose:

```sh
docker-compose up -d mongo
```

```sh
./test
```

If any of your tests fail or have new snapshots, you can review them with:

```sh
cargo insta review
```

### Time manipulation

For some snapshot tests you'll need to lock SystemTime to a fixed number to prevent things like `date_modified` or `date_created` updates to differ between snapshots. Because mongodb-base-service automatically updates the objects with those times, it has been updated to allow for mocking time. This already happens inside the `load_filled_database` function. But if you need to override it or fill the database with your own distinct mock data, you can set the time to a specific number:

```rust
// fix time to Jan 1, 2020 so that snapshots always have the same date_created, etc...
mock_time::set_mock_time(SystemTime::UNIX_EPOCH + Duration::from_millis(1577836800000));
```

Or if you need to increase the time to verify that the `date_modified` has changed:

```rust
// increase time by 10 seconds
mock_time::increase_mock_time(10000);
```

If you want to reset the time to normal SystemTime, you can use:

```rust
// revert to normal SystemTime
mock_time::clear_mock_time();
```

### Notes on multi-threading

It's important to understand that parallel tests which all write to a single database or manipulate `SystemTime` can cause issues and break tests. For that reason to ensure that the tests all run independently you should use:

```sh
cargo test --jobs=1 -- --test-threads=1
```

When you run them with concurrenncy disabled that does mean that insta will fail. In order to resolve that be sure that all tests include a snapshot name

```rust
assert_snapshot!("test_snapshot_name", format!("{:?}", resp));
```
