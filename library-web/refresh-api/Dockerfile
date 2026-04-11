FROM node:20-alpine

RUN apk add --no-cache docker-cli docker-cli-compose

WORKDIR /app/project

COPY Dockerfile ./Dockerfile
COPY docker-compose.yml ./docker-compose.yml
COPY nginx.conf ./nginx.conf
COPY package.json ./package.json
COPY package-lock.json ./package-lock.json
COPY astro.config.mjs ./astro.config.mjs
COPY tailwind.config.mjs ./tailwind.config.mjs
COPY tsconfig.json ./tsconfig.json
COPY public ./public
COPY refresh-api ./refresh-api
COPY scripts ./scripts
COPY src ./src

WORKDIR /app/project/refresh-api

CMD ["npm", "run", "start"]
