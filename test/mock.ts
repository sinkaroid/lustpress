import p from "phin";
import { load } from "cheerio";

const url = "https://www.pornhub.com/view_video.php?viewkey=ph63c4e1dc48fe7";

async function test() {
  const res = await p({
    url: url,
    "headers": {
      "User-Agent": process.env.USER_AGENT || "lustpress/1.6.0 Node.js/16.9.1",
    },
  });
    
  const $ = load(res.body);
  const title = $("meta[property='og:title']").attr("content");
  console.log(title);
  console.log(res.statusCode);
}

test().catch(console.error);
