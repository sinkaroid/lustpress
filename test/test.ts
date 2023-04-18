import c from "../src/utils/options";
import p from "phin";

for (const url of 
  [c.PORNHUB, c.XNXX, c.REDTUBE, c.XVIDEOS, c.XHAMSTER, c.YOUPORN]) {
  p({ url }).then(res => {
    if (res.statusCode !== 200) {
      console.log(`${url} is not available, status code: ${res.statusCode}, check the sites or your own user-agent`);
    }
    else {
      console.log(`${url} is available, can be scraped`);
    }
  });
}