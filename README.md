<div align="center">
<a href="http://localhost:3000/"><img width="500" src="resources/project/images/lustpress-node_1.png" alt="lustpress"></a>

<h4 align="center">RESTful and experimental API for PornHub and other R18 websites</h4>
<p align="center">
	<a href="https://github.com/sinkaroid/lustpress/actions/workflows/playground.yml"><img src="https://github.com/sinkaroid/lustpress/workflows/Playground/badge.svg"></a>
	<a href="https://codeclimate.com/github/sinkaroid/lustpress/maintainability"><img src="https://api.codeclimate.com/v1/badges/29a2be78f853f9e3a4a3/maintainability" /></a>
</p>

Lustpress stand for Lust and **Express**, rebuild from [Jandapress](https://github.com/sinkaroid/jandapress) with completely different approach.
The motivation of this project is to bring you an actionable data related to pornhub and other r18 sites with gather, similar design pattern, endpoint bindings, and consistent structure in mind.

<a href="https://sinkaroid.github.io/lustpress">Playground</a> •
<a href="https://github.com/sinkaroid/lustpress/blob/master/CONTRIBUTING.md">Contributing</a> •
<a href="https://github.com/sinkaroid/lustpress/issues/new/choose">Report Issues</a>
</div>

---

<a href="http://localhost:3000/"><img align="right" src="resources/project/images/bnnuy.png" width="320"></a>

- [Lustpress](#)
  - [The problem](#the-problem)
  - [The solution](#the-solution)
  - [Features](#features)
    - [Running tests](#Running-tests)
  - [Prerequisites](#prerequisites)
    - [Installation](#installation)
      - [Docker](#docker)
      - [Manual](#manual)
    - [Tests case](#tests)
  - [Playground](https://sinkaroid.github.io/lustpress)
    - [Routing](#playground)
    - [Status response](#status-response)
  - [CLosing remarks](https://github.com/sinkaroid/lustpress/blob/master/CLOSING_REMARKS.md)
    - [Alternative links](https://github.com/sinkaroid/lustpress/blob/master/CLOSING_REMARKS.md#alternative-links)
  - [Pronunciation](#Pronunciation)
  - [Client libraries](#client-libraries)
  - [Acknowledgements](#acknowledgements)
  - [Legal](#legal)
  - [Discontinued playground](#frequently-asked-questions)
  - [again, discontinued playground](#frequently-asked-questions)


## The problem

Many developers consume r18 websites as a source of data when building web applications. However, most of these sites — such as pornhub, redtube, and others — do not provide official APIs or public resources that can be easily integrated into applications.

As a result, developers often need to implement their own scraping logic, build multiple abstractions, and manually maintain integrations for each site.

Lustpress aims to simplify this process by providing a unified interface for accessing data across multiple r18 sites. Instead of maintaining separate implementations, developers can rely on Lustpress to reduce complexity and development overhead.

The current state of the service is **free to use**, meaning anonymous usage is allowed. No authentication is required, and **CORS is enabled** to support browser-based applications.

## The solution
Don't make it long, make it short. All processed through single rest endpoint bindings
<a href="https://sinkaroid.github.io/lustpress"><img src="resources/project/images/coverage.png" width="800"></a>

## Features

- Aggregates data from multiple sites.
- Provides a consistent and structured response format across all sources.
- Extracted objects are normalized and reassembled to support extensibility.
- Unified interface supporting **get**, **search**, and **random** methods.
- Planned support for optional **JWT authentication** in future releases.
- Primarily based on pure scraping techniques (with limited exceptions where required).

## Running tests
Some tests may fail in CI environments because certain websites restrict or block automated requests originating from CI infrastructure and shared IP ranges, but trying to keep up

| Site            | Status                                                                                                                                                                            | Get | Search | Random | Related |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | ------ | ------ | ------- |
| `pornhub`       | [![pornhub](https://github.com/sinkaroid/lustpress/workflows/Pornhub%20test/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/pornhub.yml)                   | `Yes`  | `Yes`     | `Yes`     | `Yes`     |
| `xnxx`       | [![xnxx](https://github.com/sinkaroid/lustpress/workflows/Xnxx%20test/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/xnxx.yml)                  | `Yes`  | `Yes`     | `Yes`     | `Yes`     |
| `redtube`     | [![redtube](https://github.com/sinkaroid/lustpress/workflows/Redtube%20test/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/redtube.yml)             | `Yes`  | `Yes`     | `Yes`     | `Yes`     |
| `xvideos`   | [![xvideos](https://github.com/sinkaroid/lustpress/workflows/Xvideos%20test/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/xvideos.yml)       | `Yes`  | `Yes`     | `Yes`      | `Yes`     |
| `xhamster` | [![xhamster](https://github.com/sinkaroid/lustpress/workflows/Xhamster%20test/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/xhamster.yml) | `Yes`  | `Yes`      | `Yes`      | `Yes`     |
| `youporn`     | [![youporn](https://github.com/sinkaroid/lustpress/workflows/Youporn%20test/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/youporn.yml)            | `Yes`  | `Yes`     | `Yes`     | `Yes`     |
| `eporner`     | [![Eporner test](https://github.com/sinkaroid/lustpress/actions/workflows/eporner.yml/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/eporner.yml) | `Yes` | `Yes` | `Yes` | `Yes` |
| `txxx`        | [![Txxx test](https://github.com/sinkaroid/lustpress/actions/workflows/txxx.yml/badge.svg)](https://github.com/sinkaroid/lustpress/actions/workflows/txxx.yml) | `Yes` | `Yes` | `Yes` | `Yes` |

## Prerequisites
<table>
	<td><b>NOTE:</b> NodeJS 22.x or higher</td>
</table>

To handle several requests from each website, You will also need [Redis](https://redis.io/) for persistent caching, free tier is available on [Redis Labs](https://redislabs.com/), You can also choose another adapters as we using [keyv](https://github.com/jaredwray/keyv) Key-value storage with support for multiple backends. When you choosing your own adapter, all data must be stored with `<Buffer>` type.

## Installation
Rename `.env.schema` to `.env` and fill the value with your own

```bash
# railway, fly.dev, heroku, vercel or any free service
RAILWAY = sinkaroid

# default port
PORT = 3000

# backend storage, default is redis, if not set it will consume memory storage
REDIS_URL = redis://default:somenicepassword@redis-666.c10.us-east-6-6.ec666.cloud.redislabs.com:1337

# ttl expire cache (in X hour)
EXPIRE_CACHE = 1

# you must identify your origin, if not set it will use default
USER_AGENT = "lustpress/8.0.1 Node.js/22.22.0"
```

### Docker

    docker pull ghcr.io/sinkaroid/lustpress:latest
    docker run -p 3000:3000 -d ghcr.io/sinkaroid/lustpress:latest

### Docker (adjust your own)
```bash
docker run -d \
  --name=lustpress \
  -p 3028:3000 \
  -e REDIS_URL='redis://default:somenicepassword@redis-666.c10.us-east-6-6.ec666.cloud.redislabs.com:1337' \
  -e EXPIRE_CACHE='1' \
  -e USER_AGENT='lustpress/8.0.1 Node.js/22.22.0' \
  ghcr.io/sinkaroid/lustpress:latest
```

### Manual

    git clone https://github.com/sinkaroid/lustpress.git

- Install dependencies
  - `npm install / yarn install`
- Lustpress production
  - `npm run start:prod`
- Lustpress testing and hot reload
  - `npm run start:dev`

## Tests
Run the following commands to execute tests for each supported source:

```bash
# Check whether all supported sites are available for scraping
npm run test

# Check whether ph and (maybe the others do..) do Solving challenge in their website
npm run test:mock

# Run tests for individual sources
npm run test:pornhub
npm run test:xnxx
npm run test:redtube
npm run test:xvideos
npm run test:xhamster
npm run test:youporn
npm run test:eporner
npm run test:txxx
```



### Start the production server
`npm run start:prod`

### Running development server
`npm run start:dev`

### Generating playground like swagger from apidoc definition
`npm run build:apidoc`

> To running other tests, you can see object scripts in file `package.json` or modify the `lustpress.test.ts` according your needs

## Playground
https://sinkaroid.github.io/lustpress

- These `parameter?`: means is optional

- `/` : index page

### PornHub
The missing piece of pornhub.com - https://sinkaroid.github.io/lustpress/#api-pornhub
- `/pornhub` : pornhub api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`, `?sort`
  - **related**, takes parameters : `id`
  - **random**
  - <u>sort parameters on search</u>
    - "mr", "mv", "tr", "lg"
  - Example
    - http://localhost:3000/pornhub/get?id=ph63c4e1dc48fe7
    - http://localhost:3000/pornhub/search?key=milf&page=2&sort=mr
    - http://localhost:3000/pornhub/related?id=ph63c4e1dc48fe7
    - http://localhost:3000/pornhub/random

### Xnxx
The missing piece of xnxx.com - https://sinkaroid.github.io/lustpress/#api-xnxx
- `/xnxx` : xnxx api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`, and TBD
  - **related**, takes parameters : `id`
  - **random**
  - <u>sort parameters on search</u>
    - TBD
  - Example
    - http://localhost:3000/xnxx/get?id=video-17vah71a/makima_y_denji
    - http://localhost:3000/xnxx/search?key=bbc&page=2
    - http://localhost:3000/xnxx/related?id=video-17vah71a/makima_y_denji
    - http://localhost:3000/xnxx/random

### RedTube
The missing piece of redtube.com - https://sinkaroid.github.io/lustpress/#api-redtube
- `/redtube` : redtube api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`, and TBD
  - **related**, takes parameters : `id`
  - **random**
  - <u>sort parameters on search</u>
    - TBD
  - Example
    - http://localhost:3000/redtube/get?id=42763661
    - http://localhost:3000/redtube/search?key=asian&page=2
    - http://localhost:3000/redtube/related?id=42763661
    - http://localhost:3000/redtube/random

### Xvideos
The missing piece of xvideos.com - https://sinkaroid.github.io/lustpress/#api-xvideos
- `/xvideos` : xvideos api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`, and TBD
  - **related**, takes parameters : `id`
  - **random**
  - <u>sort parameters on search</u>
    - TBD
  - Example
    - http://localhost:3000/xvideos/get?id=video73564387/cute_hentai_maid_with_pink_hair_fucking_uncensored_
    - http://localhost:3000/xvideos/search?key=hentai&page=2
    - http://localhost:3000/xvideos/related?id=video73564387/cute_hentai_maid_with_pink_hair_fucking_uncensored_
    - http://localhost:3000/xvideos/random

### Xhamster
The missing piece of xhamster.com - https://sinkaroid.github.io/lustpress/#api-xhamster
- `/xhamster` : xhamster api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`, and TBD
  - **related**, takes parameters : `id`
  - **random**
  - Example
    - http://localhost:3000/xhamster/get?id=videos/horny-makima-tests-new-toy-and-cums-intensely-xhAa5wx
    - http://localhost:3000/xhamster/search?key=arab&page=2
    - http://localhost:3000/xhamster/related?id=videos/horny-makima-tests-new-toy-and-cums-intensely-xhAa5wx
    - http://localhost:3000/xhamster/random

### YouPorn
The missing piece of youporn.com - https://sinkaroid.github.io/lustpress/#api-youporn
- `/youporn` : youporn api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`, and TBD
  - **related**, takes parameters : `id`
  - **random**
  - <u>sort parameters on search</u>
    - TBD
  - Example
    - http://localhost:3000/youporn/get?id=16621192/chainsaw-man-fuck-makima-3d-porn-60-fps
    - http://localhost:3000/youporn/search?key=teen&page=2
    - http://localhost:3000/youporn/related?id=16621192/chainsaw-man-fuck-makima-3d-porn-60-fps
    - http://localhost:3000/youporn/random

### Eporner
https://sinkaroid.github.io/lustpress/#api-eporner
- `/eporner` : eporner api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`
  - **related**, takes parameters : `id`
  - **random**
  - <u>sort parameters on search</u>
    - TBD
  - Example
    - http://localhost:3000/eporner/get?id=ibvqvezXzcs
    - http://localhost:3000/eporner/search?key=makima&page=2
    - http://localhost:3000/eporner/related?id=GPOFlSQLukS
    - http://localhost:3000/eporner/random

### Txxx
https://sinkaroid.github.io/lustpress/#api-txxx
- `/txxx` : txxx api
  - **get**, takes parameters : `id`
  - **search**, takes parameters : `key`, `?page`
  - **related**, takes parameters : `id`
  - **random**
  - Example
    - http://localhost:3000/txxx/get?id=5309183
    - http://localhost:3000/txxx/search?key=femboy&page=1
    - http://localhost:3000/txxx/related?id=7794034
    - http://localhost:3000/txxx/random


## Status response
`"success": true,` or `"success": false,`

    HTTP/1.1 200 OK
    HTTP/1.1 400 Bad Request
    HTTP/1.1 500 Fail to get data

## Frequently asked questions 
**Q: The website response is slow**  
> That's unfortunate, this repository was opensource already, You can host and deploy Lustpress with your own instance. Any fixes and improvements will updating to this repo.  

> **March 11, 2026**:
We have discontinued providing public APIs and playground services due to ongoing abuse and excessive usage.
To continue using Lustpress, please deploy and run your own self-hosted instance.


## Pronunciation
`en_US` • **/lʌstˈprɛs/** — "lust" stand for this project and "press" for express.


## Client libraries
Seamlessly integrate with the languages you love, simplified the usage, and intelisense definitions on your IDEs

- TBD
- Or [create your own](https://github.com/sinkaroid/lustpress/edit/master/README.md)


## Legal
This tool can be freely copied, modified, altered, distributed without any attribution whatsoever. However, if you feel
like this tool deserves an attribution, mention it. It won't hurt anybody.
> Licence: WTF.
