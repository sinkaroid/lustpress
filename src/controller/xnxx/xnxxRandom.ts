import { scrapeContent } from "../../scraper/xnxx/xnxxGetController";
import c from "../../utils/options";
import { logger } from "../../utils/logger";
import { maybeError } from "../../utils/modifier";
import { Request, Response } from "express";
import { load } from "cheerio";
import LustPress from "../../LustPress";

const lust = new LustPress();

export async function randomXnxx(req: Request, res: Response) {
  try {
   

    /**
     * @api {get} /xnxx/random Get random xnxx
     * @apiName Get random xnxx
     * @apiGroup xnxx
     * @apiDescription Get a random xnxx video
     * 
     * @apiSuccessExample {json} Success-Response:
     *   HTTP/1.1 200 OK
     *   HTTP/1.1 400 Bad Request
     * 
     * @apiExample {curl} curl
     * curl -i https://lust.scathach.id/xnxx/random
     * 
     * @apiExample {js} JS/TS
     * import axios from "axios"
     * 
     * axios.get("https://lust.scathach.id/xnxx/random")
     * .then(res => console.log(res.data))
     * .catch(err => console.error(err))
     * 
     * @apiExample {python} Python
     * import aiohttp
     * async with aiohttp.ClientSession() as session:
     *  async with session.get("https://lust.scathach.id/xnxx/random") as resp:
     *    print(await resp.json())
     */
    const resolve = await lust.fetchBody("https://www.xnxx.com/search/random/random");
    const $ = load(resolve);
    const search = $("div.mozaique > div")
      .map((i, el) => {
        return $(el).find("a").attr("href");
      }).get();
    const random = Math.floor(Math.random() * search.length);
    
    const url = c.XNXX + search[random];
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
