import { scrapeContent } from "../../scraper/youporn/youpornGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";
import { load } from "cheerio";
import LustPress from "../../LustPress";

const lust = new LustPress();

export async function randomYouporn(req: Request, res: Response) {
  try {
   

    /**
     * @api {get} /youporn/random Get random youporn
     * @apiName Get random youporn
     * @apiGroup youporn
     * @apiDescription Get a random youporn video
     * 
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/youporn/random
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/youporn/random")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/youporn/random") as resp:
     *    print(await resp.json())
     */
    const resolve = await lust.fetchBody(`${c.YOUPORN}`);
    const $ = load(resolve);
    const search = $("a[href^='/watch/']")
      .map((i, el) => {
        return $(el).attr("href");
      }).get();
    const random = Math.floor(Math.random() * search.length);
    const url = c.YOUPORN + search[random];
    const data = await scrapeContent(url);
    logger.info({
      path: req.path,
      query: req.query,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent")
    });
    return res.json(data);
  } catch (err) {
    const e = err as Error;
    res.status(400).json(maybeError(false, e.message));
  }
}
