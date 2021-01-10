# Rhymer

[![CI Status](../../workflows/CI/badge.svg)](../../actions)
[![Deploy Status](../../workflows/Deploy/badge.svg)](../../actions)

API server module in Rust

- [x] (Tested) User login/signup
- [x] (Tested) Object
- [x] Access control
- [x] (Tested) Hook(before/after save, before/after destroy)
- [x] Function
- [x] Github CI and github page with cargo doc
- [ ] User email

## Usage

### User


### Object


### File


### Query


### ACL


### Hook


### Function


## Development

### Initialize MongoDB

Create a user with username, password and a database of name "rhymer-test".

```javascript
// Login your admin user so that we can create new user.
use admin
db.auth(xxx, xxx);

use rhymer-test
db.createUser({
  user: 'rhymer-test',
  pwd: 'rhymer-test',
  roles: ['readWrite']
});
```

Then execute the initialization script `scripts/init-db.js` on MongoDB. For example, when using official docker image, start it and run `docker exec -i mongo mongo < scripts/init-db.js`.

## Known Issues

- [Error convert string Json to Document using Serde.](https://github.com/mongodb/bson-rust/issues/189)
