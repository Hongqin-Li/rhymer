sudo docker run -itd --name mongo -p 27017:27017 mongo
echo "Waiting for MongoDB to start..."
sleep 10
sudo docker exec -i mongo mongo rhymer-test --eval "db.createUser({user: 'rhymer-test', pwd: 'rhymer-test', roles: ['readWrite']});"
sudo docker exec -i mongo mongo < scripts/initdb.js
