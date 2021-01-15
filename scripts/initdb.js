
use rhymer-test;

db.auth("rhymer-test", "rhymer-test");

db.getCollection("_User").createIndex({"username": 1}, { unique: true, });