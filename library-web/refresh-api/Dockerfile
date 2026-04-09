FROM node:20-alpine

RUN apk add --no-cache docker-cli docker-cli-compose

WORKDIR /app

COPY refresh-api/package.json ./refresh-api/package.json
COPY refresh-api/server.mjs ./refresh-api/server.mjs

WORKDIR /app/refresh-api

CMD ["npm", "run", "start"]
