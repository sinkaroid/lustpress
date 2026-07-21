import {
  GraphQLBoolean,
  GraphQLList,
  GraphQLNonNull,
  GraphQLObjectType,
  GraphQLSchema,
  GraphQLString,
} from "graphql";

// ── Controller imports (individual files, following endpoint.ts pattern) ──

import { getPornhub } from "../controller/pornhub/pornhubGet";
import { searchPornhub } from "../controller/pornhub/pornhubSearch";
import { randomPornhub } from "../controller/pornhub/pornhubRandom";
import { relatedPornhub } from "../controller/pornhub/pornhubGetRelated";

import { getXnxx } from "../controller/xnxx/xnxxGet";
import { searchXnxx } from "../controller/xnxx/xnxxSearch";
import { randomXnxx } from "../controller/xnxx/xnxxRandom";
import { relatedXnxx } from "../controller/xnxx/xnxxGetRelated";

import { getRedtube } from "../controller/redtube/redtubeGet";
import { searchRedtube } from "../controller/redtube/redtubeSearch";
import { randomRedtube } from "../controller/redtube/redtubeRandom";
import { relatedRedtube } from "../controller/redtube/redtubeGetRelated";

import { getXvideos } from "../controller/xvideos/xvideosGet";
import { searchXvideos } from "../controller/xvideos/xvideosSearch";
import { randomXvideos } from "../controller/xvideos/xvideosRandom";
import { relatedXvideos } from "../controller/xvideos/xvideosGetRelated";

import { getXhamster } from "../controller/xhamster/xhamsterGet";
import { searchXhamster } from "../controller/xhamster/xhamsterSearch";
import { randomXhamster } from "../controller/xhamster/xhamsterRandom";
import { relatedXhamster } from "../controller/xhamster/xhamsterGetRelated";

import { getYouporn } from "../controller/youporn/youpornGet";
import { searchYouporn } from "../controller/youporn/youpornSearch";
import { randomYouporn } from "../controller/youporn/youpornRandom";
import { relatedYouporn } from "../controller/youporn/youpornGetRelated";

import { getEporner } from "../controller/eporner/epornerGet";
import { searchEporner } from "../controller/eporner/epornerSearch";
import { randomEporner } from "../controller/eporner/epornerRandom";
import { relatedEporner } from "../controller/eporner/epornerGetRelated";

import { getTxxx } from "../controller/txxx/txxxGet";
import { searchTxxx } from "../controller/txxx/txxxSearch";
import { randomTxxx } from "../controller/txxx/txxxRandom";
import { relatedTxxx } from "../controller/txxx/txxxGetRelated";


// ── Shared GraphQL types ──

const VideoDataType = new GraphQLObjectType({
  name: "VideoData",
  fields: {
    title: { type: GraphQLString },
    id: { type: GraphQLString },
    image: { type: GraphQLString },
    duration: { type: GraphQLString },
    views: { type: GraphQLString },
    rating: { type: GraphQLString },
    uploaded: { type: GraphQLString },
    upvoted: { type: GraphQLString },
    downvoted: { type: GraphQLString },
    channel: { type: GraphQLString },
    models: { type: new GraphQLList(GraphQLString) },
    tags: { type: new GraphQLList(GraphQLString) },
  },
});

const VideoResultType = new GraphQLObjectType({
  name: "VideoResult",
  fields: {
    success: { type: GraphQLBoolean },
    data: { type: VideoDataType },
    source: { type: GraphQLString },
    assets: { type: new GraphQLList(GraphQLString) },
  },
});

const SearchItemType = new GraphQLObjectType({
  name: "SearchItem",
  fields: {
    link: { type: GraphQLString },
    id: { type: GraphQLString },
    title: { type: GraphQLString },
    image: { type: GraphQLString },
    duration: { type: GraphQLString },
    rating: { type: GraphQLString },
    views: { type: GraphQLString },
    uploader: { type: GraphQLString },
    video: { type: GraphQLString },
  },
});

const SearchResultType = new GraphQLObjectType({
  name: "SearchResult",
  fields: {
    success: { type: GraphQLBoolean },
    data: { type: new GraphQLList(SearchItemType) },
    source: { type: GraphQLString },
  },
});

// ── Shared function type aliases (kept separate so eslint doesn't scan type-only params) ──

// eslint-disable-next-line no-unused-vars
type SourceGetFn = (p: { query: { id: string } }) => Promise<unknown>;
// eslint-disable-next-line no-unused-vars
type SourceSearchFn = (p: { query: { key: string; page?: string } }) => Promise<unknown>;
type SourceRandomFn = () => Promise<unknown>;

// ── Helper: build per-source query type ──

function makeSourceType(name: string, fns: {
  get?: SourceGetFn;
  search?: SourceSearchFn;
  random?: SourceRandomFn;
  related?: SourceGetFn;
}) {
  const fields: Record<string, any> = {};
  if (fns.get) {
    fields.get = {
      type: VideoResultType,
      args: { id: { type: GraphQLString } },
      resolve: async (_: unknown, a: { id: string }) =>
        fns.get!({ query: { id: a.id } }),
    };
  }
  if (fns.search) {
    fields.search = {
      type: SearchResultType,
      args: {
        key: { type: new GraphQLNonNull(GraphQLString) },
        page: { type: GraphQLString },
      },
      resolve: async (_: unknown, a: { key: string; page?: string }) =>
        fns.search!({ query: { key: a.key, page: a.page } }),
    };
  }
  if (fns.random) {
    fields.random = {
      type: VideoResultType,
      resolve: async () => fns.random!(),
    };
  }
  if (fns.related) {
    fields.related = {
      type: SearchResultType,
      args: { id: { type: GraphQLString } },
      resolve: async (_: unknown, a: { id: string }) =>
        fns.related!({ query: { id: a.id } }),
    };
  }
  return new GraphQLObjectType({ name: `${name}Queries`, fields });
}

// ── Namespaced source query types ──

const PornhubQueriesType = makeSourceType("Pornhub", {
  get: getPornhub,
  search: searchPornhub,
  random: randomPornhub,
  related: relatedPornhub,
});

const XnxxQueriesType = makeSourceType("Xnxx", {
  get: getXnxx,
  search: searchXnxx,
  random: randomXnxx,
  related: relatedXnxx,
});

const RedtubeQueriesType = makeSourceType("Redtube", {
  get: getRedtube,
  search: searchRedtube,
  random: randomRedtube,
  related: relatedRedtube,
});

const XvideosQueriesType = makeSourceType("Xvideos", {
  get: getXvideos,
  search: searchXvideos,
  random: randomXvideos,
  related: relatedXvideos,
});

const XhamsterQueriesType = makeSourceType("Xhamster", {
  get: getXhamster,
  search: searchXhamster,
  random: randomXhamster,
  related: relatedXhamster,
});

const YoupornQueriesType = makeSourceType("Youporn", {
  get: getYouporn,
  search: searchYouporn,
  random: randomYouporn,
  related: relatedYouporn,
});

const EpornerQueriesType = makeSourceType("Eporner", {
  get: getEporner,
  search: searchEporner,
  random: randomEporner,
  related: relatedEporner,
});

const TxxxQueriesType = makeSourceType("Txxx", {
  get: getTxxx,
  search: searchTxxx,
  random: randomTxxx,
  related: relatedTxxx,
});

// ── Root Query type ──

const QueryType = new GraphQLObjectType({
  name: "Query",
  fields: {
    pornhub: { type: PornhubQueriesType, resolve: () => ({}) },
    xnxx: { type: XnxxQueriesType, resolve: () => ({}) },
    redtube: { type: RedtubeQueriesType, resolve: () => ({}) },
    xvideos: { type: XvideosQueriesType, resolve: () => ({}) },
    xhamster: { type: XhamsterQueriesType, resolve: () => ({}) },
    youporn: { type: YoupornQueriesType, resolve: () => ({}) },
    eporner: { type: EpornerQueriesType, resolve: () => ({}) },
    txxx: { type: TxxxQueriesType, resolve: () => ({}) },
  },
});

export const schema = new GraphQLSchema({ query: QueryType });
