FROM node:20-alpine AS build

WORKDIR /app

# 安装依赖
COPY package.json package-lock.json* ./
RUN npm install

# 复制源码
COPY . .

# 从 GitHub 拉取馆藏内容并构建
ARG GITHUB_TOKEN
ARG GITHUB_OWNER=Kizunad
ARG GITHUB_REPO=Bong
ARG GITHUB_BRANCH=main

ENV GITHUB_TOKEN=${GITHUB_TOKEN}
ENV GITHUB_OWNER=${GITHUB_OWNER}
ENV GITHUB_REPO=${GITHUB_REPO}
ENV GITHUB_BRANCH=${GITHUB_BRANCH}

RUN npm run build

# 生产阶段 — 纯静态文件
FROM nginx:alpine

COPY --from=build /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80
