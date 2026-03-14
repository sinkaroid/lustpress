FROM node:22

WORKDIR /srv/app

COPY package*.json ./
RUN npm install

# install browser + system deps
RUN npx playwright install --with-deps chromium

COPY . .

RUN npm run build

EXPOSE 3000

CMD ["node", "build/src/index.js"]